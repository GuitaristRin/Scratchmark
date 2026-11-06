use scratchmark_core::{Document, Error, MAX_COORD, Sample};

fn point(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn commit_line(d: &mut Document, x: f64, width: f64) {
    d.begin(width, 0x11223344).unwrap();
    d.append(&[point(x, 0.0, 0), point(x + 10.0, 0.0, 1)])
        .unwrap();
    d.finish().unwrap();
}

#[test]
fn hit_test_uses_raw_centerline_width_and_tolerance() {
    let mut d = Document::default();
    d.begin(4.0, 0).unwrap();
    d.append(&[point(0.0, 0.0, 0), point(10.0, 0.0, 1)])
        .unwrap();
    d.finish().unwrap();
    let r = d.state().revision;

    assert_eq!(d.hit_test(r, 5.0, 2.0, 0.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(r, 5.0, 2.001, 0.0).unwrap(), None);
    assert_eq!(d.hit_test(r, 5.0, 3.0, 1.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(r, 5.0, 3.001, 1.0).unwrap(), None);
    assert_eq!(d.hit_test(r, 12.0, 0.0, 0.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(r, 12.001, 0.0, 0.0).unwrap(), None);
    assert_eq!(d.hit_test(r, 20.0, 0.0, 0.0).unwrap(), None);
}

#[test]
fn single_point_and_corner_hits_follow_segment_distance() {
    let mut d = Document::default();
    d.begin(4.0, 0).unwrap();
    d.append(&[point(100.0, 100.0, 0)]).unwrap();
    d.finish().unwrap();
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 102.0, 100.0, 0.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(r, 102.001, 100.0, 0.0).unwrap(), None);

    d.begin(2.0, 0).unwrap();
    d.append(&[point(0.0, 0.0, 0), point(10.0, 10.0, 1)])
        .unwrap();
    d.finish().unwrap();
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 5.0, 5.0, 0.0).unwrap(), Some(1));
    assert_eq!(d.hit_test(r, 5.0, 6.0, 0.0).unwrap(), Some(1));
    assert_eq!(d.hit_test(r, 5.0, 7.0, 0.0).unwrap(), None);
}

#[test]
fn multiple_hits_choose_the_topmost_stroke() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    commit_line(&mut d, 0.0, 2.0);
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 5.0, 0.0, 0.0).unwrap(), Some(1));

    d.erase_stroke(r, 1).unwrap();
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 5.0, 0.0, 0.0).unwrap(), Some(0));
}

#[test]
fn active_stroke_is_not_a_hit_and_hit_does_not_change_state() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    let before = d.state();
    d.begin(2.0, 0).unwrap();
    d.append(&[point(0.0, 10.0, 0)]).unwrap();
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 5.0, 10.0, 0.0).unwrap(), None);
    assert_eq!(d.hit_test(r, 5.0, 0.0, 0.0).unwrap(), Some(0));
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, before.stroke_count);
}

#[test]
fn hit_validation_is_atomic_and_reports_stale_first() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    let state = d.state();
    for (x, y, tolerance) in [
        (f64::NAN, 0.0, 0.0),
        (0.0, f64::INFINITY, 0.0),
        (0.0, 0.0, -1.0),
        (0.0, 0.0, f64::INFINITY),
        (MAX_COORD + 1.0, 0.0, 0.0),
        (0.0, 0.0, MAX_COORD + 1.0),
    ] {
        assert_eq!(
            d.hit_test(state.revision, x, y, tolerance),
            Err(Error::InvalidArgument)
        );
        assert_eq!(d.state(), state);
    }
    assert_eq!(
        d.hit_test(state.revision - 1, f64::NAN, 0.0, 0.0),
        Err(Error::StaleRevision)
    );
    assert_eq!(d.state(), state);
}

#[test]
fn empty_document_and_empty_lookup_return_no_hit() {
    let mut d = Document::default();
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 0.0, 0.0, 10.0).unwrap(), None);
    commit_line(&mut d, 0.0, 2.0);
    let r = d.state().revision;
    assert_eq!(d.hit_test(r, 1000.0, 1000.0, 0.0).unwrap(), None);
}

#[test]
fn erase_restores_original_z_order_through_undo_and_redo() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    commit_line(&mut d, 100.0, 2.0);
    commit_line(&mut d, 200.0, 2.0);
    let r = d.state().revision;

    assert_eq!(d.erase_stroke(r + 1, 1), Err(Error::StaleRevision));
    assert_eq!(d.erase_stroke(r, 3), Err(Error::InvalidArgument));
    assert_eq!(d.state().revision, r);

    d.erase_stroke(r, 1).unwrap();
    let r = d.state().revision;
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(d.hit_test(r, 105.0, 0.0, 0.0).unwrap(), None);
    assert_eq!(d.stroke_info(r, 1).unwrap().min_x, 199.0);

    d.undo().unwrap();
    let r = d.state().revision;
    assert_eq!(d.state().stroke_count, 3);
    assert_eq!(d.hit_test(r, 105.0, 0.0, 0.0).unwrap(), Some(1));
    assert_eq!(d.stroke_info(r, 1).unwrap().min_x, 99.0);

    d.redo().unwrap();
    let r = d.state().revision;
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(d.hit_test(r, 105.0, 0.0, 0.0).unwrap(), None);
    assert_eq!(d.stroke_info(r, 1).unwrap().min_x, 199.0);
}

#[test]
fn erase_rejects_active_stroke_and_does_not_mutate_on_invalid_index() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    d.begin(2.0, 0).unwrap();
    let r = d.state().revision;
    assert_eq!(d.erase_stroke(r, 0), Err(Error::InvalidState));
    assert_eq!(d.state().stroke_count, 1);
    d.cancel().unwrap();

    let r = d.state().revision;
    assert_eq!(d.erase_stroke(r, 1), Err(Error::InvalidArgument));
    assert_eq!(d.state().revision, r);
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn erase_clears_redo_and_new_edits_do_not_resurrect_dropped_strokes() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    commit_line(&mut d, 100.0, 2.0);
    d.undo().unwrap();
    assert_eq!(d.state().can_redo, 1);

    let r = d.state().revision;
    d.erase_stroke(r, 0).unwrap();
    assert_eq!(d.state().stroke_count, 0);
    assert_eq!(d.state().can_redo, 0);

    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    assert_eq!(d.stroke_info(d.state().revision, 0).unwrap().min_x, -1.0);
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 0);

    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 0);
}

#[test]
fn commit_after_erase_undo_clears_redo_and_history_survives_save() {
    let mut d = Document::default();
    commit_line(&mut d, 0.0, 2.0);
    let r = d.state().revision;
    d.erase_stroke(r, 0).unwrap();
    d.undo().unwrap();
    assert_eq!(d.state().can_redo, 1);

    d.begin(2.0, 0).unwrap();
    d.append(&[point(50.0, 0.0, 0)]).unwrap();
    d.finish().unwrap();
    assert_eq!(d.state().can_redo, 0);
    assert_eq!(d.state().stroke_count, 2);

    let erased = {
        let r = d.state().revision;
        d.erase_stroke(r, 0).unwrap();
        d.save().unwrap()
    };
    let mut reopened = Document::default();
    reopened.load(&erased).unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
    assert_eq!(reopened.state().can_undo, 1);
    assert_eq!(reopened.state().can_redo, 0);
    assert_eq!(
        reopened
            .hit_test(reopened.state().revision, 0.0, 0.0, 0.0)
            .unwrap(),
        None
    );
    assert_eq!(
        reopened
            .hit_test(reopened.state().revision, 50.0, 0.0, 0.0)
            .unwrap(),
        Some(0)
    );
    reopened.undo().unwrap();
    assert_eq!(reopened.state().stroke_count, 0);
    reopened.redo().unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
}
