use scratchmark_core::{Document, Error, Sample, ShapeKind};

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.5,
        flags: 1,
    }
}

fn commit(d: &mut Document, width: f64, points: &[(f64, f64)]) {
    d.begin(width, 0x11223344).unwrap();
    let samples: Vec<Sample> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| sample(*x, *y, i as u64 * 100))
        .collect();
    d.append(&samples).unwrap();
    d.finish().unwrap();
}

fn points_of(d: &Document, index: u32) -> Vec<Sample> {
    let revision = d.state().revision;
    let info = d.stroke_info(revision, index).unwrap();
    let mut buffer = vec![sample(0.0, 0.0, 0); info.point_count as usize];
    let written = d.read_points(revision, index, 0, &mut buffer).unwrap();
    buffer.truncate(written);
    buffer
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

#[test]
fn line_fit_snaps_noisy_horizontal_stroke() {
    let mut d = Document::default();
    // 关于 x 轴对称的噪声：主方向仍严格是 x 轴，便于断言端点。
    let noisy = [
        (0.0, 0.1),
        (2.5, -0.1),
        (5.0, 0.0),
        (7.5, -0.1),
        (10.0, 0.1),
    ];
    commit(&mut d, 2.0, &noisy);
    let revision = d.state().revision;

    let info = d
        .regularize_stroke(revision, 0, ShapeKind::Line, false)
        .unwrap();
    assert_eq!(info.kind, ShapeKind::Line as u32);
    assert_eq!(info.stroke_count, 1);
    assert_eq!(info.point_count, 2);
    assert!(info.error > 0.0 && info.error < 0.15);

    let points = points_of(&d, 0);
    assert_eq!(points.len(), 2);
    close(points[0].x, 0.0, 1e-9);
    close(points[0].y, 0.0, 1e-9);
    close(points[1].x, 10.0, 1e-9);
    close(points[1].y, 0.0, 1e-9);
    // 规整后的样本不带真实压力。
    assert_eq!(points[0].flags, 0);
    assert_eq!(points[0].pressure, 0.0);
    // 时间在原始首末之间且非递减。
    assert_eq!(points[0].time_us, 0);
    assert_eq!(points[1].time_us, 400);
}

#[test]
fn line_fit_handles_vertical_and_diagonal_strokes() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[(5.0, 0.0), (5.0, 4.0), (5.0, 9.0)]);
    let revision = d.state().revision;
    d.regularize_stroke(revision, 0, ShapeKind::Line, false)
        .unwrap();
    let points = points_of(&d, 0);
    close(points[0].x, 5.0, 1e-9);
    close(points[0].y, 0.0, 1e-9);
    close(points[1].x, 5.0, 1e-9);
    close(points[1].y, 9.0, 1e-9);

    commit(&mut d, 2.0, &[(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)]);
    let revision = d.state().revision;
    let info = d
        .regularize_stroke(revision, 1, ShapeKind::Line, false)
        .unwrap();
    assert!(info.error < 1e-9);
    let points = points_of(&d, 1);
    close(points[0].x, 0.0, 1e-9);
    close(points[1].x, 10.0, 1e-9);
    close(points[1].y, 10.0, 1e-9);
}

#[test]
fn degenerate_strokes_are_rejected_per_kind() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(5.0, 5.0, 0)]).unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;
    assert_eq!(
        d.regularize_stroke(revision, 0, ShapeKind::Line, false),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.regularize_stroke(revision, 0, ShapeKind::Circle, false),
        Err(Error::InvalidArgument)
    );

    // 两个完全相同的点：轴线无定义。
    commit(&mut d, 2.0, &[(3.0, 3.0), (3.0, 3.0)]);
    let revision = d.state().revision;
    assert_eq!(
        d.regularize_stroke(revision, 1, ShapeKind::Line, false),
        Err(Error::InvalidArgument)
    );

    // 水平线的高为 0：不能做矩形。
    commit(&mut d, 2.0, &[(0.0, 0.0), (10.0, 0.0)]);
    let revision = d.state().revision;
    assert_eq!(
        d.regularize_stroke(revision, 2, ShapeKind::Rectangle, false),
        Err(Error::InvalidArgument)
    );

    // 共线点：圆拟合奇异。
    commit(
        &mut d,
        2.0,
        &[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (15.0, 0.0)],
    );
    let revision = d.state().revision;
    assert_eq!(
        d.regularize_stroke(revision, 3, ShapeKind::Circle, false),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn rectangle_uses_axis_aligned_box_and_closes_the_loop() {
    let mut d = Document::default();
    commit(
        &mut d,
        3.0,
        &[
            (0.0, 0.0),
            (10.0, 0.5),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 5.0),
        ],
    );
    let revision = d.state().revision;
    let info = d
        .regularize_stroke(revision, 0, ShapeKind::Rectangle, false)
        .unwrap();
    assert_eq!(info.point_count, 5);
    let points = points_of(&d, 0);
    assert_eq!(points.len(), 5);
    close(points[0].x, 0.0, 1e-9);
    close(points[0].y, 0.0, 1e-9);
    close(points[1].x, 10.0, 1e-9);
    close(points[2].y, 10.0, 1e-9);
    // 闭合：末点等于首点。
    close(points[4].x, points[0].x, 1e-12);
    close(points[4].y, points[0].y, 1e-12);
    assert_eq!(d.stroke_info(d.state().revision, 0).unwrap().width, 3.0);
    assert!(info.error < 0.5);
}

#[test]
fn circle_fit_recovers_center_radius_and_closes() {
    let mut d = Document::default();
    let center = (100.0, 50.0);
    let radius = 20.0;
    let points: Vec<(f64, f64)> = (0..32)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / 32.0;
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect();
    commit(&mut d, 2.0, &points);
    let revision = d.state().revision;

    let info = d
        .regularize_stroke(revision, 0, ShapeKind::Circle, false)
        .unwrap();
    assert_eq!(info.stroke_count, 1);
    assert_eq!(info.point_count, 65);
    assert!(info.error < 1e-9, "error {}", info.error);

    let circle = points_of(&d, 0);
    assert_eq!(circle.len(), 65);
    for point in circle.iter() {
        let dx = point.x - center.0;
        let dy = point.y - center.1;
        close((dx * dx + dy * dy).sqrt(), radius, 1e-9);
    }
    close(circle[64].x, circle[0].x, 1e-9);
    close(circle[64].y, circle[0].y, 1e-9);
}

#[test]
fn arrow_emits_shaft_and_two_head_strokes_at_the_same_index() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)]);
    commit(&mut d, 2.0, &[(0.0, 200.0), (10.0, 200.0)]);
    let revision = d.state().revision;

    let info = d
        .regularize_stroke(revision, 0, ShapeKind::Arrow, false)
        .unwrap();
    assert_eq!(info.stroke_count, 3);
    assert_eq!(info.point_count, 6);
    assert!(info.error < 1e-9);
    assert_eq!(d.state().stroke_count, 4);

    // 箭杆仍是原索引，头两笔紧随其后；后面的笔划顺延。
    let shaft = points_of(&d, 0);
    close(shaft[0].x, 0.0, 1e-9);
    close(shaft[1].x, 100.0, 1e-9);

    let head = points_of(&d, 1);
    assert_eq!(head.len(), 2);
    close(head[0].x, 100.0, 1e-9);
    close(head[0].y, 0.0, 1e-9);
    let expected = 0.18 * 100.0;
    close(head[1].x, 100.0 - expected * 28f64.to_radians().cos(), 1e-9);
    close(head[1].y, expected * 28f64.to_radians().sin(), 1e-9);

    let other = points_of(&d, 3);
    close(other[0].y, 200.0, 1e-9);
}

#[test]
fn preview_is_read_only_and_reports_the_same_numbers() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[(0.0, 0.0), (5.0, 1.0), (10.0, 0.0)]);
    let before = d.state();

    let preview = d
        .regularize_stroke(before.revision, 0, ShapeKind::Line, true)
        .unwrap();
    assert_eq!(d.state(), before);
    assert_eq!(d.state().can_undo, 1);
    assert_eq!(d.stroke_info(before.revision, 0).unwrap().point_count, 3);

    // 预览不要求空闲：活动笔划期间也能算。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(50.0, 50.0, 0)]).unwrap();
    let active = d.state().revision;
    let during = d
        .regularize_stroke(active, 0, ShapeKind::Line, true)
        .unwrap();
    assert_eq!(during.point_count, preview.point_count);
    // 实际替换则要求空闲。
    assert_eq!(
        d.regularize_stroke(active, 0, ShapeKind::Line, false),
        Err(Error::InvalidState)
    );
    d.cancel().unwrap();
}

#[test]
fn regularize_is_one_undo_entry_and_keeps_metadata() {
    let mut d = Document::default();
    commit(&mut d, 3.0, &[(0.0, 0.2), (5.0, -0.2), (10.0, 0.2)]);
    let original = points_of(&d, 0);
    let revision = d.state().revision;

    d.regularize_stroke(revision, 0, ShapeKind::Line, false)
        .unwrap();
    let info = d.stroke_info(d.state().revision, 0).unwrap();
    assert_eq!(info.width, 3.0);
    assert_eq!(info.color, 0x11223344);
    assert_eq!(d.state().can_undo, 1);

    d.undo().unwrap();
    assert_eq!(points_of(&d, 0), original);
    d.redo().unwrap();
    assert_eq!(d.stroke_info(d.state().revision, 0).unwrap().point_count, 2);

    // 未知索引与过期 revision 都被拒绝。
    let revision = d.state().revision;
    assert_eq!(
        d.regularize_stroke(revision, 5, ShapeKind::Line, true),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.regularize_stroke(revision - 1, 0, ShapeKind::Line, true),
        Err(Error::StaleRevision)
    );
}

#[test]
fn regularized_shapes_survive_snapshot_round_trip() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[(0.0, 0.0), (10.0, 1.0), (20.0, 0.0)]);
    let revision = d.state().revision;
    d.regularize_stroke(revision, 0, ShapeKind::Arrow, false)
        .unwrap();
    let bytes = d.save().unwrap();

    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.state().stroke_count, 3);
    let shaft = points_of(&reopened, 0);
    close(shaft[1].x, 20.0, 1e-9);
    assert_eq!(reopened.state().can_redo, 0);
    // load 后每笔各有一条“新增”历史，撤销按逆序逐笔移除。
    reopened.undo().unwrap();
    assert_eq!(reopened.state().stroke_count, 2);
}
