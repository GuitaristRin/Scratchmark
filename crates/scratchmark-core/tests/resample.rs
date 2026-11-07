use scratchmark_core::{BrushOptions, Document, Error, ResampleOptions, Sample};

fn flat_brush() -> BrushOptions {
    BrushOptions {
        alpha: 1.0,
        min_width_ratio: 1.0,
        pressure_enabled: 0,
        reserved: 0,
    }
}

fn options(ratio: f64) -> ResampleOptions {
    ResampleOptions {
        spacing_ratio: ratio,
        brush: flat_brush(),
        reserved: 0,
    }
}

fn sample(x: f64, y: f64, time_us: u64, pressure: f32, flags: u32) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure,
        flags,
    }
}

/// 一笔直线：从 (0,0) 到 (10,0)，线宽 2。
fn straight(d: &mut Document) -> u64 {
    d.begin(2.0, 0x11223344).unwrap();
    d.append(&[sample(0.0, 0.0, 0, 0.0, 0), sample(10.0, 0.0, 100, 0.0, 0)])
        .unwrap();
    let revision = d.state().revision;
    d.finish().unwrap();
    revision
}

#[test]
fn spacing_follows_width_and_keeps_the_end_point() {
    let mut d = Document::default();
    let revision = straight(&mut d);
    let revision = d.state().revision.max(revision);

    // 线宽 2、比例 0.5 → 间距 1：0..10 共 11 个点。
    let points = d.resample_stroke(revision, 0, &options(0.5)).unwrap();
    assert_eq!(points.len(), 11);
    for (index, point) in points.iter().enumerate() {
        assert!(
            (point.x - index as f64).abs() < 1e-9,
            "第 {index} 点 x={}",
            point.x
        );
        assert!(point.y.abs() < 1e-9);
        assert!((point.width - 2.0).abs() < 1e-9);
    }

    // 比例 0.3 → 间距 0.6：0, 0.6, …, 9.6 共 17 点，再补末点 10 → 18 点。
    let points = d.resample_stroke(revision, 0, &options(0.3)).unwrap();
    assert_eq!(points.len(), 18);
    assert!((points[0].x - 0.0).abs() < 1e-9);
    assert!((points[1].x - 0.6).abs() < 1e-9);
    assert!((points[16].x - 9.6).abs() < 1e-9);
    assert!((points[17].x - 10.0).abs() < 1e-9);

    // 比例 1.25 → 间距 2.5：0, 2.5, 5, 7.5, 10 → 5 点（末点正好在步点上，不重复）。
    let points = d.resample_stroke(revision, 0, &options(1.25)).unwrap();
    assert_eq!(points.len(), 5);
    assert!((points[4].x - 10.0).abs() < 1e-9);
}

#[test]
fn wider_stroke_yields_wider_spacing_and_fewer_points() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(0.0, 0.0, 0, 0.0, 0), sample(10.0, 0.0, 100, 0.0, 0)])
        .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    let thin = d.resample_stroke(revision, 0, &options(0.5)).unwrap();

    // 同样比例、线宽加倍 → 间距加倍 → 点数约减半。
    d.begin(4.0, 0).unwrap();
    d.append(&[
        sample(0.0, 20.0, 0, 0.0, 0),
        sample(10.0, 20.0, 100, 0.0, 0),
    ])
    .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    let thick = d.resample_stroke(revision, 1, &options(0.5)).unwrap();
    assert!(thick.len() < thin.len());
    assert_eq!(thick.len(), 6); // 间距 2，0..10 共 6 点
}

#[test]
fn corner_strokes_stay_continuous_without_duplicate_vertices() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    // L 形：先向右 10，再向上 10；线宽 2、间距 1。
    d.append(&[
        sample(0.0, 0.0, 0, 0.0, 0),
        sample(10.0, 0.0, 100, 0.0, 0),
        sample(10.0, 10.0, 200, 0.0, 0),
    ])
    .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;

    let points = d.resample_stroke(revision, 0, &options(0.5)).unwrap();
    // 总长 20、间距 1 → 21 个点。
    assert_eq!(points.len(), 21);
    // 第 10 点正好是拐角，只出现一次。
    assert!((points[10].x - 10.0).abs() < 1e-9);
    assert!((points[10].y - 0.0).abs() < 1e-9);
    assert!((points[11].y - 1.0).abs() < 1e-9);
    assert!((points[20].y - 10.0).abs() < 1e-9);
}

#[test]
fn single_point_and_degenerate_strokes_return_one_point() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(5.0, 5.0, 0, 0.0, 0)]).unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    let points = d.resample_stroke(revision, 0, &options(0.5)).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!((points[0].x, points[0].y), (5.0, 5.0));

    // 所有点重合：同样一个点。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(1.0, 1.0, 0, 0.0, 0), sample(1.0, 1.0, 1, 0.0, 0)])
        .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    let points = d.resample_stroke(revision, 1, &options(0.5)).unwrap();
    assert_eq!(points.len(), 1);
}

#[test]
fn pressure_widths_are_interpolated_along_the_stroke() {
    let mut d = Document::default();
    d.begin(10.0, 0).unwrap();
    d.append(&[sample(0.0, 0.0, 0, 0.0, 1), sample(10.0, 0.0, 100, 1.0, 1)])
        .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    let brush = BrushOptions {
        alpha: 1.0,
        min_width_ratio: 0.2,
        pressure_enabled: 1,
        reserved: 0,
    };
    let spacing_ratio = 0.3; // 间距 3 → 0, 3, 6, 9, 10
    let points = d
        .resample_stroke(
            revision,
            0,
            &ResampleOptions {
                spacing_ratio,
                brush,
                reserved: 0,
            },
        )
        .unwrap();
    assert_eq!(points.len(), 5);
    // 与 M2 派生点在相邻位置线性插值：第一个点压力 0 → 宽度 = 10 × (0.2 + 0.8×0) = 2。
    assert!(
        (points[0].width - 2.0).abs() < 1e-6,
        "width {}",
        points[0].width
    );
    // 末点压力 1 → 宽度 = 10。
    assert!(
        (points[4].width - 10.0).abs() < 1e-6,
        "width {}",
        points[4].width
    );
    // 中间点随位置递增。
    assert!(points[1].width > points[0].width);
}

#[test]
fn invalid_options_and_bad_indexes_are_rejected() {
    let mut d = Document::default();
    straight(&mut d);
    let revision = d.state().revision;

    for ratio in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            d.resample_stroke(revision, 0, &options(ratio)),
            Err(Error::InvalidArgument)
        );
    }
    // 非法笔刷配置。
    let bad_brush = ResampleOptions {
        spacing_ratio: 0.5,
        brush: BrushOptions {
            alpha: 0.0,
            min_width_ratio: 1.0,
            pressure_enabled: 0,
            reserved: 0,
        },
        reserved: 0,
    };
    assert_eq!(
        d.resample_stroke(revision, 0, &bad_brush),
        Err(Error::InvalidArgument)
    );
    // reserved 必须为 0。
    let bad_reserved = ResampleOptions {
        reserved: 1,
        ..options(0.5)
    };
    assert_eq!(
        d.resample_stroke(revision, 0, &bad_reserved),
        Err(Error::InvalidArgument)
    );
    // 索引与 revision。
    assert_eq!(
        d.resample_stroke(revision, 9, &options(0.5)),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.resample_stroke(revision + 1, 0, &options(0.5)),
        Err(Error::StaleRevision)
    );
    // 只读：不改变文档。
    assert_eq!(d.state().revision, revision);
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn tiny_spacing_can_hit_the_point_limit() {
    let mut d = Document::default();
    // 长 1e6、线宽 1e6 且比例极小 → 点数必然超上限。
    d.begin(1e6, 0).unwrap();
    d.append(&[sample(0.0, 0.0, 0, 0.0, 0), sample(1e6, 0.0, 100, 0.0, 0)])
        .unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    // 间距 = 1e6 × 1e-9 = 1e-3 → 约 1e9 个点，远超 MAX_POINTS。
    assert_eq!(
        d.resample_stroke(revision, 0, &options(1e-9)),
        Err(Error::LimitExceeded)
    );
}
