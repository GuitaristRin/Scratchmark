use scratchmark_core::{Document, Error, Sample, SampleAxes};

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn line(d: &mut Document, from: (f64, f64), to: (f64, f64), width: f64) {
    d.begin(width, 0x11223344).unwrap();
    d.append(&[sample(from.0, from.1, 0), sample(to.0, to.1, 10)])
        .unwrap();
    d.finish().unwrap();
}

#[test]
fn document_bounds_unions_committed_strokes_and_includes_width() {
    let mut d = Document::default();
    assert!(d.document_bounds().is_none(), "空文档没有边界");

    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let bounds = d.document_bounds().unwrap();
    // 线宽 2：每侧各 1。
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, -1.0));
    assert_eq!((bounds.max_x, bounds.max_y), (11.0, 1.0));

    line(&mut d, (100.0, 40.0), (110.0, 50.0), 4.0);
    let bounds = d.document_bounds().unwrap();
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, -1.0));
    assert_eq!((bounds.max_x, bounds.max_y), (112.0, 52.0));

    // 活动笔划不参与。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(999.0, 999.0, 0)]).unwrap();
    let bounds = d.document_bounds().unwrap();
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, -1.0));
    assert_eq!((bounds.max_x, bounds.max_y), (112.0, 52.0));
    d.cancel().unwrap();
}

#[test]
fn translate_all_moves_every_stroke_as_one_undo_entry() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 100.0), (120.0, 120.0), 3.0);
    let revision = d.state().revision;

    d.translate_all(revision, 0.0, 50.0).unwrap();
    let bounds = d.document_bounds().unwrap();
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, 49.0));
    assert_eq!((bounds.max_x, bounds.max_y), (121.5, 171.5));

    // 一次撤销全部回到原位。
    d.undo().unwrap();
    let bounds = d.document_bounds().unwrap();
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, -1.0));
    assert_eq!((bounds.max_x, bounds.max_y), (121.5, 121.5));
    d.redo().unwrap();
    assert_eq!(d.document_bounds().unwrap().min_y, 49.0);
}

#[test]
fn translate_all_keeps_width_axes_and_snapshot() {
    let mut d = Document::default();
    d.begin(4.0, 0x11223344).unwrap();
    let samples = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 10)];
    let axes = [
        SampleAxes {
            tilt: 0.3,
            orientation: 1.0,
        },
        SampleAxes {
            tilt: 0.6,
            orientation: -1.0,
        },
    ];
    d.append_with_axes(&samples, Some(&axes)).unwrap();
    d.finish().unwrap();
    let revision = d.state().revision;

    d.translate_all(revision, 5.0, 7.0).unwrap();
    let info = d.stroke_info(d.state().revision, 0).unwrap();
    assert_eq!(info.width, 4.0);
    assert_eq!(info.color, 0x11223344);
    let mut read = [SampleAxes::default(); 2];
    assert_eq!(d.read_axes(d.state().revision, 0, 0, &mut read).unwrap(), 2);
    assert_eq!(read, axes);

    // 快照往返仍是平移后的坐标。
    let bytes = d.save().unwrap();
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    // 平移 +7、线宽 4：min_y = 0 + 7 - 2 = 5。
    assert_eq!(reopened.document_bounds().unwrap().min_y, 5.0);
}

#[test]
fn translate_all_rejects_bad_state_revision_and_domain_overflow() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let state = d.state();

    // 空文档：空操作，不递增 revision。
    let mut empty = Document::default();
    let empty_revision = empty.state().revision;
    empty.translate_all(empty_revision, 10.0, 10.0).unwrap();
    assert_eq!(empty.state().revision, empty_revision);

    // 参数与状态。
    assert_eq!(
        d.translate_all(state.revision, f64::NAN, 0.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.translate_all(state.revision, 0.0, 1e9 + 1.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.translate_all(state.revision + 1, 1.0, 1.0),
        Err(Error::StaleRevision)
    );
    // 越界：整次失败，文档不变。
    assert_eq!(
        d.translate_all(state.revision, 1e9, 0.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(d.state(), state);

    // 活动笔划期间拒绝。
    d.begin(2.0, 0).unwrap();
    let active = d.state().revision;
    assert_eq!(d.translate_all(active, 1.0, 1.0), Err(Error::InvalidState));
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn erasing_after_translate_keeps_history_consistent() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let revision = d.state().revision;
    d.translate_all(revision, 0.0, 100.0).unwrap();

    // 平移后命中测试仍在平移后的位置生效。
    let revision = d.state().revision;
    assert_eq!(d.hit_test(revision, 5.0, 100.0, 0.0).unwrap(), Some(0));
    assert_eq!(d.hit_test(revision, 5.0, 0.0, 0.0).unwrap(), None);

    d.erase_stroke(revision, 0).unwrap();
    assert_eq!(d.state().stroke_count, 0);
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    // 撤销删除后仍在平移后的位置。
    assert_eq!(d.document_bounds().unwrap().min_y, 99.0);
    d.undo().unwrap();
    // 再撤销一次回到平移前。
    assert_eq!(d.document_bounds().unwrap().min_y, -1.0);
}
