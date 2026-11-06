use scratchmark_core::{Document, Error, MAX_COORD, Point, Sample};

fn point(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn pressure_point(x: f64, y: f64, time_us: u64, pressure: f32) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure,
        flags: 1,
    }
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn commit(d: &mut Document, width: f64, samples: &[Sample]) {
    d.begin(width, 0x01020304).unwrap();
    d.append(samples).unwrap();
    d.finish().unwrap();
}

fn points_of(d: &Document, index: u32) -> Vec<Sample> {
    let revision = d.state().revision;
    let info = d.stroke_info(revision, index).unwrap();
    let mut buffer = vec![point(0.0, 0.0, 0); info.point_count as usize];
    let written = d.read_points(revision, index, 0, &mut buffer).unwrap();
    buffer.truncate(written);
    buffer
}

fn xs(d: &Document, index: u32) -> Vec<f64> {
    points_of(d, index).iter().map(|s| s.x).collect()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn perpendicular_cut_splits_stroke_with_interpolated_points() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1000)]);
    let revision = d.state().revision;

    d.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();
    let state = d.state();
    assert_eq!(state.stroke_count, 2);
    assert_ne!(state.revision, revision);
    assert_eq!(state.can_undo, 1);
    assert_eq!(state.can_redo, 0);

    let cut_revision = state.revision;
    let first = points_of(&d, 0);
    assert_eq!(first.len(), 2);
    close(first[1].x, 3.0);
    assert_eq!(first[1].time_us, 300);
    let second = points_of(&d, 1);
    assert_eq!(second.len(), 2);
    close(second[0].x, 7.0);
    assert_eq!(second[0].time_us, 700);
    close(second[1].x, 10.0);

    // 片段继承宽度与颜色。
    let info = d.stroke_info(cut_revision, 0).unwrap();
    assert_eq!(info.width, 2.0);
    assert_eq!(info.color, 0x01020304);
}

#[test]
fn cut_points_interpolate_time_pressure_and_flags() {
    let mut d = Document::default();
    commit(
        &mut d,
        2.0,
        &[
            pressure_point(0.0, 0.0, 0, 0.0),
            pressure_point(10.0, 0.0, 100, 1.0),
        ],
    );
    let revision = d.state().revision;
    d.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();

    let end = points_of(&d, 0)[1];
    assert_eq!(end.flags, 1);
    close(end.x, 3.0);
    assert!((end.pressure - 0.3).abs() < 1e-6, "{}", end.pressure);
    assert_eq!(end.time_us, 30);

    let start = points_of(&d, 1)[0];
    assert_eq!(start.flags, 1);
    close(start.x, 7.0);
    assert!((start.pressure - 0.7).abs() < 1e-6, "{}", start.pressure);
    assert_eq!(start.time_us, 70);

    // 一端没有真实压力时，切点为 flags=0 / pressure=0，保持样本有效。
    let mut mixed = Document::default();
    commit(
        &mut mixed,
        2.0,
        &[pressure_point(0.0, 0.0, 0, 1.0), point(10.0, 0.0, 100)],
    );
    let revision = mixed.state().revision;
    mixed.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();
    let cut = points_of(&mixed, 0)[1];
    assert_eq!(cut.flags, 0);
    assert_eq!(cut.pressure, 0.0);
}

#[test]
fn tangent_touch_does_not_split_and_does_not_touch_history() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1000)]);
    let before = d.state();

    // 相切：点到路径的距离恰好等于半径，参数区间长度为 0，不切分。
    d.erase_path(before.revision, &[at(5.0, 2.0)], 2.0).unwrap();
    assert_eq!(d.state(), before);

    // 完全不在附近：同样是无效果的成功调用。
    d.erase_path(before.revision, &[at(5.0, 2.001)], 2.0)
        .unwrap();
    assert_eq!(d.state(), before);
    assert_eq!(d.stroke_info(before.revision, 0).unwrap().point_count, 2);
}

#[test]
fn fully_covered_stroke_disappears_and_undo_restores_it() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1000)]);
    let revision = d.state().revision;

    d.erase_path(revision, &[at(5.0, 0.0)], 100.0).unwrap();
    assert_eq!(d.state().stroke_count, 0);

    d.undo().unwrap();
    let state = d.state();
    assert_eq!(state.stroke_count, 1);
    assert_eq!(xs(&d, 0), vec![0.0, 10.0]);

    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 0);
}

#[test]
fn one_gesture_across_two_strokes_is_a_single_undo_entry() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    commit(&mut d, 2.0, &[point(20.0, 0.0, 0), point(30.0, 0.0, 1)]);
    commit(&mut d, 2.0, &[point(0.0, 100.0, 0), point(10.0, 100.0, 1)]);
    let revision = d.state().revision;

    // 竖直穿过 y=0 的两处，横向连接段离笔划 5 个单位，不受半径 1 影响。
    d.erase_path(
        revision,
        &[at(5.0, -5.0), at(5.0, 5.0), at(25.0, 5.0), at(25.0, -5.0)],
        1.0,
    )
    .unwrap();
    let state = d.state();
    assert_eq!(state.stroke_count, 5);
    assert_eq!(xs(&d, 0), vec![0.0, 4.0]);
    assert_eq!(xs(&d, 1), vec![6.0, 10.0]);
    assert_eq!(xs(&d, 2), vec![20.0, 24.0]);
    assert_eq!(xs(&d, 3), vec![26.0, 30.0]);
    // 未被影响的笔划保持原索引顺序。
    assert_eq!(xs(&d, 4), vec![0.0, 10.0]);

    // 一次撤销恢复两笔原样，z 序与索引回到手势之前。
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 3);
    assert_eq!(xs(&d, 0), vec![0.0, 10.0]);
    assert_eq!(xs(&d, 1), vec![20.0, 30.0]);
    assert_eq!(xs(&d, 2), vec![0.0, 10.0]);

    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 5);
    assert_eq!(xs(&d, 2), vec![20.0, 24.0]);
}

#[test]
fn repeated_cuts_on_one_stroke_keep_path_order() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(30.0, 0.0, 3000)]);
    let revision = d.state().revision;

    let path = [
        at(5.0, -5.0),
        at(5.0, 5.0),
        at(15.0, 5.0),
        at(15.0, -5.0),
        at(25.0, -5.0),
        at(25.0, 5.0),
    ];
    d.erase_path(revision, &path, 1.0).unwrap();

    assert_eq!(d.state().stroke_count, 4);
    assert_eq!(xs(&d, 0), vec![0.0, 4.0]);
    assert_eq!(xs(&d, 1), vec![6.0, 14.0]);
    assert_eq!(xs(&d, 2), vec![16.0, 24.0]);
    assert_eq!(xs(&d, 3), vec![26.0, 30.0]);
}

#[test]
fn corner_vertex_coverage_keeps_surviving_pieces_of_both_segments() {
    let mut d = Document::default();
    commit(
        &mut d,
        2.0,
        &[
            point(0.0, 0.0, 0),
            point(10.0, 0.0, 1000),
            point(10.0, 10.0, 2000),
        ],
    );
    let revision = d.state().revision;

    d.erase_path(revision, &[at(10.0, 0.0)], 3.0).unwrap();
    assert_eq!(d.state().stroke_count, 2);

    let first = points_of(&d, 0);
    assert_eq!(first.len(), 2);
    close(first[0].x, 0.0);
    close(first[1].x, 7.0);
    assert_eq!(first[1].time_us, 700);

    let second = points_of(&d, 1);
    assert_eq!(second.len(), 2);
    close(second[0].y, 3.0);
    close(second[1].y, 10.0);
    assert_eq!(second[0].time_us, 1300);
}

#[test]
fn single_point_strokes_are_all_or_nothing() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(5.0, 5.0, 0)]);
    commit(&mut d, 2.0, &[point(50.0, 50.0, 0)]);
    let revision = d.state().revision;

    d.erase_path(revision, &[at(5.0, 5.0)], 1.0).unwrap();
    assert_eq!(d.state().stroke_count, 1);
    assert_eq!(xs(&d, 0), vec![50.0]);

    // 只覆盖另一个单点笔划之外时不变。
    let revision = d.state().revision;
    d.erase_path(revision, &[at(5.0, 5.0)], 1.0).unwrap();
    assert_eq!(d.state().revision, revision);
}

#[test]
fn empty_document_and_no_effect_calls_do_not_change_state() {
    let mut d = Document::default();
    let before = d.state();
    d.erase_path(before.revision, &[at(0.0, 0.0), at(10.0, 10.0)], 1.0)
        .unwrap();
    assert_eq!(d.state(), before);
    assert_eq!(d.state().can_undo, 0);

    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    let before = d.state();
    d.erase_path(before.revision, &[at(1000.0, 1000.0)], 5.0)
        .unwrap();
    assert_eq!(d.state(), before);
}

#[test]
fn validation_is_atomic_and_reports_state_and_revision_first() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    let state = d.state();

    for (path, radius) in [
        (Vec::new(), 1.0),
        (vec![at(f64::NAN, 0.0)], 1.0),
        (vec![at(0.0, f64::INFINITY)], 1.0),
        (vec![at(MAX_COORD + 1.0, 0.0)], 1.0),
        (vec![at(0.0, 0.0)], 0.0),
        (vec![at(0.0, 0.0)], -1.0),
        (vec![at(0.0, 0.0)], f64::INFINITY),
        (vec![at(0.0, 0.0)], MAX_COORD + 1.0),
    ] {
        assert_eq!(
            d.erase_path(state.revision, &path, radius),
            Err(Error::InvalidArgument)
        );
        assert_eq!(d.state(), state);
    }

    assert_eq!(
        d.erase_path(state.revision - 1, &[at(5.0, 0.0)], 1.0),
        Err(Error::StaleRevision)
    );
    assert_eq!(d.state(), state);

    d.begin(2.0, 0).unwrap();
    d.append(&[point(5.0, 0.0, 0)]).unwrap();
    let active = d.state();
    assert_eq!(
        d.erase_path(active.revision, &[at(5.0, 0.0)], 1.0),
        Err(Error::InvalidState)
    );
    assert_eq!(d.state(), active);
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn successful_erase_clears_redo_history() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    commit(&mut d, 2.0, &[point(20.0, 0.0, 0), point(30.0, 0.0, 1)]);
    d.undo().unwrap();
    assert_eq!(d.state().can_redo, 1);

    let revision = d.state().revision;
    d.erase_path(revision, &[at(5.0, 0.0)], 1.0).unwrap();
    assert_eq!(d.state().can_redo, 0);
    assert_eq!(d.state().stroke_count, 2);

    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    assert_eq!(xs(&d, 0), vec![0.0, 10.0]);
}

#[test]
fn snapshot_after_partial_erase_reloads_fragments_and_allows_undo() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    let revision = d.state().revision;
    d.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();
    let bytes = d.save().unwrap();

    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.state().stroke_count, 2);
    assert_eq!(reopened.state().can_redo, 0);
    assert_eq!(reopened.state().can_undo, 1);
    assert_eq!(xs(&reopened, 0), vec![0.0, 3.0]);

    // load 后每笔都有一条“新增”历史，撤销按逆序逐笔移除。
    reopened.undo().unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
    assert_eq!(xs(&reopened, 0), vec![0.0, 3.0]);
    reopened.redo().unwrap();
    assert_eq!(reopened.state().stroke_count, 2);
    assert_eq!(xs(&reopened, 1), vec![7.0, 10.0]);
}

#[test]
fn erased_middle_segment_separates_fragments_on_both_sides() {
    let mut d = Document::default();
    // Π 形：两条竖边 + 一条被完全覆盖的横边。
    commit(
        &mut d,
        2.0,
        &[
            point(0.0, 0.0, 0),
            point(0.0, 10.0, 1000),
            point(10.0, 10.0, 2000),
            point(10.0, 0.0, 3000),
        ],
    );
    let revision = d.state().revision;

    d.erase_path(revision, &[at(0.0, 10.0), at(10.0, 10.0)], 2.0)
        .unwrap();
    assert_eq!(d.state().stroke_count, 2);

    let first = points_of(&d, 0);
    assert_eq!(first.len(), 2);
    close(first[1].y, 8.0);
    assert_eq!(first[1].time_us, 800);

    let second = points_of(&d, 1);
    assert_eq!(second.len(), 2);
    close(second[0].y, 8.0);
    close(second[1].y, 0.0);
    assert_eq!(second[0].time_us, 2200);
}

#[test]
fn duplicate_consecutive_samples_do_not_break_the_split() {
    let mut d = Document::default();
    commit(
        &mut d,
        2.0,
        &[point(0.0, 0.0, 0), point(0.0, 0.0, 1), point(10.0, 0.0, 2)],
    );
    let revision = d.state().revision;

    d.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(xs(&d, 0), vec![0.0, 3.0]);
    assert_eq!(xs(&d, 1), vec![7.0, 10.0]);
}

#[test]
fn fragments_are_hit_testable_and_untouched_strokes_keep_working() {
    let mut d = Document::default();
    commit(&mut d, 2.0, &[point(0.0, 0.0, 0), point(10.0, 0.0, 1)]);
    let revision = d.state().revision;
    d.erase_path(revision, &[at(5.0, 0.0)], 2.0).unwrap();
    let revision = d.state().revision;

    assert_eq!(d.hit_test(revision, 1.5, 0.0, 0.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(revision, 5.0, 0.0, 0.0).unwrap(), None);
    assert_eq!(d.hit_test(revision, 8.5, 0.0, 0.0).unwrap(), Some(1));

    // 片段仍可整笔擦除，历史顺序为“切分 → 删除”。
    d.erase_stroke(revision, 0).unwrap();
    assert_eq!(d.state().stroke_count, 1);
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 2);
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 2);
}
