use scratchmark_core::{Document, Error, Sample, SampleAxes};

fn line(d: &mut Document, y: f64) {
    d.begin(2.0, 0x11223344).unwrap();
    d.append(&[
        Sample {
            x: 0.0,
            y,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        },
        Sample {
            x: 10.0,
            y,
            time_us: 10,
            pressure: 0.0,
            flags: 0,
        },
    ])
    .unwrap();
    d.finish().unwrap();
}

#[test]
fn cut_returns_clipboard_and_removes_in_one_undo_entry() {
    let mut d = Document::default();
    line(&mut d, 0.0);
    line(&mut d, 20.0);
    line(&mut d, 40.0);
    let revision = d.state().revision;

    // 剪掉中间一笔。
    let blob = d.cut_strokes(revision, &[1]).unwrap();
    assert!(!blob.is_empty());
    assert_eq!(&blob[..8], b"SCRATCH\0");
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(d.stroke_info(d.state().revision, 1).unwrap().min_y, 39.0);

    // 一次撤销：笔划回到原位，且顺序不变。
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 3);
    assert_eq!(d.stroke_info(d.state().revision, 1).unwrap().min_y, 19.0);
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 2);
}

#[test]
fn cut_multiple_indices_restores_exactly_once() {
    let mut d = Document::default();
    for index in 0..5 {
        line(&mut d, index as f64 * 10.0);
    }
    let revision = d.state().revision;

    // 一次剪掉第 0 与第 2 笔。
    let blob = d.cut_strokes(revision, &[0, 2]).unwrap();
    assert_eq!(d.state().stroke_count, 3);
    // 剩下的是原来的第 1、3、4 笔。
    assert_eq!(d.stroke_info(d.state().revision, 0).unwrap().min_y, 9.0);
    assert_eq!(d.stroke_info(d.state().revision, 2).unwrap().min_y, 39.0);

    // 剪贴板里是刚才剪掉的两笔。
    let mut pasted = Document::default();
    pasted.paste(pasted.state().revision, &blob, None).unwrap();
    assert_eq!(pasted.state().stroke_count, 2);
    assert_eq!(
        pasted
            .stroke_info(pasted.state().revision, 0)
            .unwrap()
            .min_y,
        -1.0
    );
    assert_eq!(
        pasted
            .stroke_info(pasted.state().revision, 1)
            .unwrap()
            .min_y,
        19.0
    );

    // 一次撤销全部回来。
    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 5);
    for index in 0..5 {
        assert_eq!(
            d.stroke_info(d.state().revision, index as u32)
                .unwrap()
                .min_y,
            index as f64 * 10.0 - 1.0
        );
    }
}

#[test]
fn cut_keeps_axes_in_the_clipboard() {
    let mut d = Document::default();
    d.begin(4.0, 0x11223344).unwrap();
    let samples = [
        Sample {
            x: 0.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        },
        Sample {
            x: 10.0,
            y: 0.0,
            time_us: 10,
            pressure: 0.0,
            flags: 0,
        },
    ];
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
    line(&mut d, 100.0);
    let revision = d.state().revision;

    let blob = d.cut_strokes(revision, &[0]).unwrap();
    // 带轴向的剪切：剪贴板是格式 3、页面标识为 0。
    assert_eq!(&blob[..8], b"SCRATCH3");
    assert_eq!(&blob[16..24], &0u64.to_le_bytes());

    // 粘贴回去轴向仍在。
    let revision = d.state().revision;
    d.paste(revision, &blob, None).unwrap();
    let mut read = [SampleAxes::default(); 2];
    assert_eq!(d.read_axes(d.state().revision, 1, 0, &mut read).unwrap(), 2);
    assert_eq!(read, axes);
}

#[test]
fn cut_rejects_bad_input_without_changing_the_document() {
    let mut d = Document::default();
    line(&mut d, 0.0);
    line(&mut d, 20.0);
    let state = d.state();

    // 空列表、越界、过期 revision、活动笔划期间。
    assert_eq!(
        d.cut_strokes(state.revision, &[]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.cut_strokes(state.revision, &[2]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.cut_strokes(state.revision + 1, &[0]),
        Err(Error::StaleRevision)
    );
    assert_eq!(d.state(), state);

    d.begin(2.0, 0).unwrap();
    let active = d.state().revision;
    assert_eq!(d.cut_strokes(active, &[0]), Err(Error::InvalidState));
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, 2);

    // 重复下标按既有选区规则被拒绝（与 M6 / M7 一致），文档不变。
    let revision = d.state().revision;
    assert_eq!(
        d.cut_strokes(revision, &[0, 0]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(d.state().revision, revision);
}
