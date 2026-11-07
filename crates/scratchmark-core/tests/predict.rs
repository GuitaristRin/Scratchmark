use scratchmark_core::{
    BrushOptions, Document, Error, PredictionOptions, PredictionOptionsEx, Sample,
};

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

fn flat_brush() -> BrushOptions {
    BrushOptions {
        alpha: 1.0,
        min_width_ratio: 1.0,
        pressure_enabled: 0,
        reserved: 0,
    }
}

fn active_line(d: &mut Document) {
    d.begin(2.0, 0).unwrap();
    // 0.1 页面单位 / 微秒。
    d.append(&[
        Sample {
            x: 0.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        },
        Sample {
            x: 50.0,
            y: 0.0,
            time_us: 500,
            pressure: 0.0,
            flags: 0,
        },
        Sample {
            x: 100.0,
            y: 0.0,
            time_us: 1000,
            pressure: 0.0,
            flags: 0,
        },
    ])
    .unwrap();
}

fn base(horizon_us: f64, refresh_rate_hz: f64) -> PredictionOptionsEx {
    PredictionOptionsEx {
        horizon_us,
        // 上限给足：本组测试关心时间窗，不关心长度收缩。
        max_distance: 10_000.0,
        count: 4,
        reserved: 0,
        brush: flat_brush(),
        refresh_rate_hz,
    }
}

#[test]
fn zero_refresh_rate_matches_the_m10_result_exactly() {
    let mut d = Document::default();
    active_line(&mut d);
    let revision = d.state().revision;

    let legacy = d
        .predict(
            revision,
            &PredictionOptions {
                horizon_us: 1000.0,
                max_distance: 1000.0,
                count: 4,
                reserved: 0,
                brush: flat_brush(),
            },
        )
        .unwrap();
    let extended = d.predict_ex(revision, &base(1000.0, 0.0)).unwrap();
    assert_eq!(legacy, extended);
    assert_eq!(extended.len(), 4);
}

#[test]
fn refresh_rate_caps_the_effective_horizon() {
    let mut d = Document::default();
    active_line(&mut d);
    let revision = d.state().revision;

    // 16 ms 窗、240 Hz → 一帧 4166.67 微秒；预测长度约为原来的 0.26。
    let long = d.predict_ex(revision, &base(16_000.0, 0.0)).unwrap();
    let capped = d.predict_ex(revision, &base(16_000.0, 240.0)).unwrap();
    assert_eq!(long.len(), 4);
    assert_eq!(capped.len(), 4);
    // 笔尖在 x = 100，速度 0.1/µs：16 ms → +1600；一帧（240 Hz ≈ 4166.67 µs）→ +416.67。
    assert!((long[3].x - 1700.0).abs() < 1e-6, "x {}", long[3].x);
    assert!(
        (capped[3].x - 516.666_666_7).abs() < 1e-6,
        "x {}",
        capped[3].x
    );
    assert!(capped[3].x < long[3].x);

    // 刷新率极高时由刷新率决定：10000 Hz → 100 微秒 → +10。
    let fast = d.predict_ex(revision, &base(16_000.0, 10_000.0)).unwrap();
    assert!((fast[3].x - 110.0).abs() < 1e-6, "x {}", fast[3].x);

    // 时间窗小于一帧时仍以时间窗为准：50 微秒 → +5。
    let short = d.predict_ex(revision, &base(50.0, 240.0)).unwrap();
    assert!((short[3].x - 105.0).abs() < 1e-6, "x {}", short[3].x);
}

#[test]
fn max_distance_and_domain_rules_still_apply() {
    let mut d = Document::default();
    active_line(&mut d);
    let revision = d.state().revision;

    // 一帧很长但长度上限很小：总位移被收缩到上限。
    let mut options = base(16_000.0, 60.0);
    options.max_distance = 5.0;
    let points = d.predict_ex(revision, &options).unwrap();
    assert_eq!(points.len(), 4);
    assert!((points[3].x - 105.0).abs() < 1e-6, "x {}", points[3].x);
}

#[test]
fn invalid_refresh_rates_and_options_are_rejected() {
    let mut d = Document::default();
    active_line(&mut d);
    let revision = d.state().revision;

    for rate in [-1.0, f64::NAN, f64::INFINITY, 10_001.0] {
        assert_eq!(
            d.predict_ex(revision, &base(16_000.0, rate)),
            Err(Error::InvalidArgument)
        );
    }
    // M10 的既有非法组合仍然被拒绝。
    let mut bad = base(16_000.0, 240.0);
    bad.count = 0;
    assert_eq!(d.predict_ex(revision, &bad), Err(Error::InvalidArgument));
    let mut bad = base(16_000.0, 240.0);
    bad.horizon_us = 0.0;
    assert_eq!(d.predict_ex(revision, &bad), Err(Error::InvalidArgument));
    let mut bad = base(16_000.0, 240.0);
    bad.max_distance = 0.0;
    assert_eq!(d.predict_ex(revision, &bad), Err(Error::InvalidArgument));
    let mut bad = base(16_000.0, 240.0);
    bad.reserved = 1;
    assert_eq!(d.predict_ex(revision, &bad), Err(Error::InvalidArgument));
    assert_eq!(
        d.predict_ex(revision + 1, &base(16_000.0, 240.0)),
        Err(Error::StaleRevision)
    );

    // 有效窗计算本身可单独核对。
    assert_eq!(base(16_000.0, 0.0).effective_horizon().unwrap(), 16_000.0);
    assert!((base(16_000.0, 1000.0).effective_horizon().unwrap() - 1000.0).abs() < 1e-9);
    assert!((base(500.0, 240.0).effective_horizon().unwrap() - 500.0).abs() < 1e-9);
}

#[test]
fn no_active_stroke_still_yields_no_points() {
    let mut d = Document::default();
    assert!(
        d.predict_ex(d.state().revision, &base(16_000.0, 240.0))
            .unwrap()
            .is_empty()
    );
}
