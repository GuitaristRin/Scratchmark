use scratchmark_core::{BrushOptions, Document, Error, PredictionOptions, Sample};

fn options(count: u32, horizon_us: f64, max_distance: f64) -> PredictionOptions {
    PredictionOptions {
        horizon_us,
        max_distance,
        count,
        reserved: 0,
        brush: BrushOptions {
            alpha: 1.0,
            min_width_ratio: 1.0,
            pressure_enabled: 0,
            reserved: 0,
        },
    }
}

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn begin_straight(d: &mut Document, width: f64) {
    d.begin(width, 0x11223344).unwrap();
    // 100 页面单位 / 1000 微秒 = 0.1 单位每微秒。
    d.append(&[
        sample(0.0, 0.0, 0),
        sample(50.0, 0.0, 500),
        sample(100.0, 0.0, 1000),
    ])
    .unwrap();
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn straight_stroke_extrapolates_along_its_velocity() {
    let mut d = Document::default();
    begin_straight(&mut d, 2.0);
    let revision = d.state().revision;
    let predicted = d.predict(revision, &options(4, 1000.0, 1000.0)).unwrap();
    assert_eq!(predicted.len(), 4);
    // 速度 0.1 单位/微秒 × 1000 微秒 = 100 单位，分 4 段。
    for (i, point) in predicted.iter().enumerate() {
        close(point.x, 100.0 + 25.0 * (i + 1) as f64);
        close(point.y, 0.0);
        close(point.width, 2.0);
    }
    // 预测不改变文档。
    assert_eq!(d.state().revision, revision);
    assert_eq!(d.state().active_points, 3);
}

#[test]
fn turning_stroke_uses_the_blended_velocity() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    // 前 3 点向右，最后一点向右上：尾段速度与最近两点速度不同。
    d.append(&[
        sample(0.0, 0.0, 0),
        sample(10.0, 0.0, 100),
        sample(20.0, 0.0, 200),
        sample(30.0, 10.0, 300),
    ])
    .unwrap();
    let revision = d.state().revision;
    let predicted = d.predict(revision, &options(1, 100.0, 1000.0)).unwrap();
    assert_eq!(predicted.len(), 1);
    // 尾段速度 = (30,10)/300；最近两点 = (10,10)/100。
    let tail = (30.0 / 300.0, 10.0 / 300.0);
    let pair = (10.0 / 100.0, 10.0 / 100.0);
    let vx = 0.5 * (tail.0 + pair.0);
    let vy = 0.5 * (tail.1 + pair.1);
    close(predicted[0].x, 30.0 + vx * 100.0);
    close(predicted[0].y, 10.0 + vy * 100.0);
}

#[test]
fn max_distance_caps_the_prediction_length() {
    let mut d = Document::default();
    begin_straight(&mut d, 2.0);
    let revision = d.state().revision;
    // 不设上限时会预测 100 单位；这里限制到 10。
    let predicted = d.predict(revision, &options(5, 1000.0, 10.0)).unwrap();
    assert_eq!(predicted.len(), 5);
    close(predicted[4].x, 110.0);
    for (i, point) in predicted.iter().enumerate() {
        close(point.x, 100.0 + 2.0 * (i + 1) as f64);
    }
}

#[test]
fn no_active_stroke_or_degenerate_timing_yields_no_points() {
    let mut d = Document::default();
    // 没有活动笔划。
    let empty = d.predict(d.state().revision, &options(4, 1000.0, 100.0));
    assert!(empty.unwrap().is_empty());

    // 单点活动笔划。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(5.0, 5.0, 0)]).unwrap();
    assert!(
        d.predict(d.state().revision, &options(4, 1000.0, 100.0))
            .unwrap()
            .is_empty()
    );

    // 时间跨度为零。
    d.append(&[sample(6.0, 6.0, 0)]).unwrap();
    assert!(
        d.predict(d.state().revision, &options(4, 1000.0, 100.0))
            .unwrap()
            .is_empty()
    );
    d.cancel().unwrap();
}

#[test]
fn invalid_options_and_stale_revision_are_rejected() {
    let mut d = Document::default();
    begin_straight(&mut d, 2.0);
    let revision = d.state().revision;

    for bad in [
        options(0, 1000.0, 100.0),
        options(65, 1000.0, 100.0),
        options(4, 0.0, 100.0),
        options(4, f64::NAN, 100.0),
        options(4, 1000.0, 0.0),
        options(4, 1000.0, f64::INFINITY),
        PredictionOptions {
            reserved: 1,
            ..options(4, 1000.0, 100.0)
        },
        PredictionOptions {
            brush: BrushOptions {
                alpha: 0.0,
                min_width_ratio: 1.0,
                pressure_enabled: 0,
                reserved: 0,
            },
            ..options(4, 1000.0, 100.0)
        },
    ] {
        assert_eq!(d.predict(revision, &bad), Err(Error::InvalidArgument));
    }
    assert_eq!(
        d.predict(revision + 1, &options(4, 1000.0, 100.0)),
        Err(Error::StaleRevision)
    );
    // 活动笔划仍在，且预测失败不影响它。
    assert_eq!(d.state().active_points, 3);
}

#[test]
fn prediction_stops_at_the_page_domain_edge() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    // 笔尖贴着页面域上界，速度把它继续推向界外。
    d.append(&[
        sample(1e9 - 3.0, 0.0, 0),
        sample(1e9 - 2.0, 0.0, 100),
        sample(1e9 - 1.0, 0.0, 200),
    ])
    .unwrap();
    let revision = d.state().revision;
    let predicted = d.predict(revision, &options(8, 10.0, 100.0)).unwrap();
    // 速度 0.01/微秒 × 10 = 0.1，8 段共 0.8，仍在域内。
    assert_eq!(predicted.len(), 8);
    for point in &predicted {
        assert!(point.x <= 1e9);
    }

    // 把时间窗拉大：越界处以截断代替报错。
    let predicted = d.predict(revision, &options(8, 1000.0, 100.0)).unwrap();
    assert!(predicted.len() < 8);
    for point in &predicted {
        assert!(point.x <= 1e9);
    }
}

#[test]
fn predicted_width_matches_the_last_brush_point() {
    let mut d = Document::default();
    d.begin(10.0, 0).unwrap();
    d.append(&[
        Sample {
            x: 0.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.2,
            flags: 1,
        },
        Sample {
            x: 10.0,
            y: 0.0,
            time_us: 100,
            pressure: 0.9,
            flags: 1,
        },
    ])
    .unwrap();
    let brush = BrushOptions {
        alpha: 1.0,
        min_width_ratio: 0.25,
        pressure_enabled: 1,
        reserved: 0,
    };
    let mut predicted = options(2, 100.0, 100.0);
    predicted.brush = brush;

    let revision = d.state().revision;
    let points = d.predict(revision, &predicted).unwrap();
    assert_eq!(points.len(), 2);

    let mut brush_points = vec![
        scratchmark_core::BrushPoint {
            x: 0.0,
            y: 0.0,
            width: 0.0
        };
        2
    ];
    let written = d
        .read_brush_points(revision, 0, 0, brush, &mut brush_points)
        .unwrap();
    assert_eq!(written, 2);
    close(points[0].width, brush_points[1].width);
    close(points[1].width, brush_points[1].width);
    // 两支笔宽不同，说明确实取的是最后一点。
    assert!((brush_points[0].width - brush_points[1].width).abs() > 1e-6);
}

#[test]
fn prediction_is_read_only_for_history_and_snapshot() {
    let mut d = Document::default();
    begin_straight(&mut d, 2.0);
    d.finish().unwrap();
    let before = d.state();
    let snapshot = d.save().unwrap();

    // 已提交后没有活动笔划，预测为空。
    assert!(
        d.predict(before.revision, &options(4, 1000.0, 100.0))
            .unwrap()
            .is_empty()
    );
    assert_eq!(d.state(), before);
    assert_eq!(d.save().unwrap(), snapshot);
    assert_eq!(d.state().can_undo, 1);
}
