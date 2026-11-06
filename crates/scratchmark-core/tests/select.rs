use scratchmark_core::{Document, Error, Point, Sample};

fn point(x: f64, y: f64) -> Sample {
    Sample {
        x,
        y,
        time_us: 0,
        pressure: 0.0,
        flags: 0,
    }
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn line(d: &mut Document, from: (f64, f64), to: (f64, f64), width: f64) {
    d.begin(width, 0x11223344).unwrap();
    d.append(&[point(from.0, from.1), point(to.0, to.1)])
        .unwrap();
    d.finish().unwrap();
}

fn square() -> Vec<Point> {
    vec![
        at(100.0, 100.0),
        at(300.0, 100.0),
        at(300.0, 300.0),
        at(100.0, 300.0),
    ]
}

fn min_xs(d: &Document) -> Vec<f64> {
    let revision = d.state().revision;
    (0..d.state().stroke_count)
        .map(|index| d.stroke_info(revision, index).unwrap().min_x)
        .collect()
}

#[test]
fn lasso_selects_contained_and_crossing_strokes_in_z_order() {
    let mut d = Document::default();
    line(&mut d, (150.0, 150.0), (200.0, 150.0), 2.0); // 完全内含
    line(&mut d, (50.0, 200.0), (150.0, 200.0), 2.0); // 跨过左边界
    line(&mut d, (500.0, 500.0), (600.0, 500.0), 2.0); // 完全在外

    let revision = d.state().revision;
    assert_eq!(d.lasso_hits(revision, &square()).unwrap(), vec![0, 1]);
    // 查询不改变文档。
    assert_eq!(d.state().revision, revision);
    assert_eq!(d.state().can_undo, 1);

    // 空文档返回空集合。
    let empty = Document::default();
    assert!(
        empty
            .lasso_hits(empty.state().revision, &square())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn boundary_contact_and_collinear_overlap_count_as_selected() {
    let mut d = Document::default();
    // 沿多边形左边线走：共线重叠。
    line(&mut d, (100.0, 120.0), (100.0, 260.0), 2.0);
    // 端点正好落在边界上。
    line(&mut d, (300.0, 200.0), (400.0, 200.0), 2.0);
    // 只差一点，完全在外。
    line(&mut d, (99.0, 400.0), (50.0, 400.0), 2.0);

    let revision = d.state().revision;
    assert_eq!(d.lasso_hits(revision, &square()).unwrap(), vec![0, 1]);
}

#[test]
fn concave_polygon_does_not_select_strokes_in_its_notch() {
    let mut d = Document::default();
    // L 形：(0,0)-(300,0)-(300,100)-(100,100)-(100,300)-(0,300)
    let lasso = vec![
        at(0.0, 0.0),
        at(300.0, 0.0),
        at(300.0, 100.0),
        at(100.0, 100.0),
        at(100.0, 300.0),
        at(0.0, 300.0),
    ];
    line(&mut d, (50.0, 50.0), (250.0, 50.0), 2.0); // 在横条内
    line(&mut d, (50.0, 250.0), (50.0, 150.0), 2.0); // 在竖条内
    line(&mut d, (200.0, 200.0), (280.0, 280.0), 2.0); // 在缺口里

    let revision = d.state().revision;
    assert_eq!(d.lasso_hits(revision, &lasso).unwrap(), vec![0, 1]);
}

#[test]
fn single_point_strokes_use_containment_only() {
    let mut d = Document::default();
    d.begin(4.0, 0).unwrap();
    d.append(&[point(200.0, 200.0)]).unwrap();
    d.finish().unwrap();
    d.begin(4.0, 0).unwrap();
    d.append(&[point(400.0, 200.0)]).unwrap();
    d.finish().unwrap();

    let revision = d.state().revision;
    assert_eq!(d.lasso_hits(revision, &square()).unwrap(), vec![0]);
}

#[test]
fn lasso_skips_active_stroke_and_validates_input() {
    let mut d = Document::default();
    line(&mut d, (150.0, 150.0), (200.0, 150.0), 2.0);
    d.begin(2.0, 0).unwrap();
    d.append(&[point(180.0, 180.0), point(190.0, 190.0)])
        .unwrap();
    let active = d.state().revision;
    assert_eq!(d.lasso_hits(active, &square()).unwrap(), vec![0]);
    d.cancel().unwrap();

    let revision = d.state().revision;
    for bad in [
        vec![at(100.0, 100.0), at(300.0, 100.0)],
        vec![at(f64::NAN, 0.0), at(0.0, 0.0), at(1.0, 1.0)],
        vec![at(0.0, f64::INFINITY), at(0.0, 0.0), at(1.0, 1.0)],
        vec![at(1e9 + 1.0, 0.0), at(0.0, 0.0), at(1.0, 1.0)],
    ] {
        assert_eq!(d.lasso_hits(revision, &bad), Err(Error::InvalidArgument));
    }
    assert_eq!(
        d.lasso_hits(revision - 1, &square()),
        Err(Error::StaleRevision)
    );
    assert_eq!(d.state().revision, revision);
}

#[test]
fn self_intersecting_lasso_is_deterministic() {
    let mut d = Document::default();
    line(&mut d, (120.0, 120.0), (140.0, 140.0), 2.0);
    let bowtie = vec![
        at(0.0, 0.0),
        at(200.0, 200.0),
        at(200.0, 0.0),
        at(0.0, 200.0),
    ];
    let revision = d.state().revision;
    let first = d.lasso_hits(revision, &bowtie).unwrap();
    let second = d.lasso_hits(revision, &bowtie).unwrap();
    assert_eq!(first, second);
}

#[test]
fn erase_indices_is_one_undo_entry_and_restores_z_order() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    line(&mut d, (200.0, 0.0), (210.0, 0.0), 2.0);
    line(&mut d, (300.0, 0.0), (310.0, 0.0), 2.0);
    let revision = d.state().revision;

    // 索引顺序任意，但必须全部有效且不重复。
    d.erase_indices(revision, &[3, 1]).unwrap();
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(min_xs(&d), vec![-1.0, 199.0]);

    // 一次撤销恢复全部并回到原 z 序。
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 4);
    assert_eq!(min_xs(&d), vec![-1.0, 99.0, 199.0, 299.0]);

    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(min_xs(&d), vec![-1.0, 199.0]);
}

#[test]
fn erase_indices_rejects_duplicates_out_of_range_and_bad_state() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    let state = d.state();

    assert_eq!(
        d.erase_indices(state.revision, &[0, 0]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.erase_indices(state.revision, &[0, 2]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.erase_indices(state.revision - 1, &[0]),
        Err(Error::StaleRevision)
    );
    assert_eq!(d.state(), state);

    // 空列表是空操作：不改 revision、不产生历史。
    d.erase_indices(state.revision, &[]).unwrap();
    assert_eq!(d.state().revision, state.revision);

    d.begin(2.0, 0).unwrap();
    let active = d.state().revision;
    assert_eq!(d.erase_indices(active, &[0]), Err(Error::InvalidState));
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, 2);
}

#[test]
fn erase_indices_clears_redo_and_snapshot_keeps_survivors() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    d.undo().unwrap();
    assert_eq!(d.state().can_redo, 1);

    d.erase_indices(d.state().revision, &[0]).unwrap();
    assert_eq!(d.state().can_redo, 0);
    assert_eq!(d.state().stroke_count, 0);

    line(&mut d, (200.0, 0.0), (210.0, 0.0), 2.0);
    let bytes = d.save().unwrap();
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
    assert_eq!(min_xs(&reopened), vec![199.0]);
}

#[test]
fn clear_is_one_undo_and_noop_on_empty_document() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    line(&mut d, (200.0, 0.0), (210.0, 0.0), 2.0);
    let revision = d.state().revision;

    d.clear(revision).unwrap();
    assert_eq!(d.state().stroke_count, 0);
    assert_eq!(d.state().can_undo, 1);

    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 3);
    assert_eq!(min_xs(&d), vec![-1.0, 99.0, 199.0]);
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 0);

    // 空文档：OK 且 revision 不变。
    d.undo().unwrap();
    d.clear(d.state().revision).unwrap();
    let cleared = d.state();
    d.clear(cleared.revision).unwrap();
    assert_eq!(d.state().revision, cleared.revision);
    assert_eq!(d.clear(cleared.revision - 1), Err(Error::StaleRevision));

    d.begin(2.0, 0).unwrap();
    let active = d.state().revision;
    assert_eq!(d.clear(active), Err(Error::InvalidState));
    d.cancel().unwrap();
}

#[test]
fn lasso_then_erase_end_to_end_and_undo() {
    let mut d = Document::default();
    line(&mut d, (150.0, 150.0), (200.0, 150.0), 2.0);
    line(&mut d, (50.0, 200.0), (150.0, 200.0), 2.0);
    line(&mut d, (500.0, 500.0), (600.0, 500.0), 2.0);

    let revision = d.state().revision;
    let hits = d.lasso_hits(revision, &square()).unwrap();
    assert_eq!(hits, vec![0, 1]);
    d.erase_indices(revision, &hits).unwrap();
    assert_eq!(d.state().stroke_count, 1);
    assert_eq!(min_xs(&d), vec![499.0]);

    d.undo().unwrap();
    assert_eq!(min_xs(&d), vec![149.0, 49.0, 499.0]);
}
