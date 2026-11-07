use scratchmark_core::{Document, Error, FLAG_CUT_END, FLAG_PRESSURE, Point, Sample, ShapeKind};

fn point(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn commit(d: &mut Document, samples: &[Sample]) {
    d.begin(2.0, 0x11223344).unwrap();
    d.append(samples).unwrap();
    d.finish().unwrap();
}

fn points_of(d: &Document, index: u32) -> Vec<Sample> {
    let revision = d.state().revision;
    let count = d.stroke_info(revision, index).unwrap().point_count as usize;
    let mut buffer = vec![point(0.0, 0.0, 0); count];
    let written = d.read_points(revision, index, 0, &mut buffer).unwrap();
    buffer.truncate(written);
    buffer
}

/// 一笔从 (0,0) 到 (10,0)，在中间以半径 2 擦一下 → 两段。
fn cut_in_two() -> Document {
    let mut d = Document::default();
    commit(&mut d, &[point(0.0, 0.0, 0), point(10.0, 0.0, 100)]);
    let revision = d.state().revision;
    d.erase_path(revision, &[Point { x: 5.0, y: 0.0 }], 2.0)
        .unwrap();
    d
}

#[test]
fn cut_points_are_marked_but_original_ends_are_not() {
    let d = cut_in_two();
    assert_eq!(d.state().stroke_count, 2);

    let first = points_of(&d, 0);
    // 首点来自原始笔划，不带切点标记；末点是切出来的，带标记。
    assert_eq!(first[0].flags & FLAG_CUT_END, 0);
    assert_ne!(first[1].flags & FLAG_CUT_END, 0);

    let second = points_of(&d, 1);
    assert_ne!(second[0].flags & FLAG_CUT_END, 0);
    assert_eq!(second[1].flags & FLAG_CUT_END, 0);
}

#[test]
fn pressure_bit_survives_alongside_the_cut_bit() {
    let mut d = Document::default();
    commit(
        &mut d,
        &[
            Sample {
                pressure: 0.2,
                flags: FLAG_PRESSURE,
                ..point(0.0, 0.0, 0)
            },
            Sample {
                pressure: 0.9,
                flags: FLAG_PRESSURE,
                ..point(10.0, 0.0, 100)
            },
        ],
    );
    let revision = d.state().revision;
    d.erase_path(revision, &[Point { x: 5.0, y: 0.0 }], 2.0)
        .unwrap();

    let cut = points_of(&d, 0)[1];
    // 两端都带压力 → 压力位保留，并加上切点位。
    assert_eq!(cut.flags, FLAG_PRESSURE | FLAG_CUT_END);
    assert!(cut.pressure > 0.0 && cut.pressure < 1.0);
}

#[test]
fn several_cuts_mark_every_cut_boundary() {
    let mut d = Document::default();
    commit(&mut d, &[point(0.0, 0.0, 0), point(30.0, 0.0, 300)]);
    let revision = d.state().revision;
    // 两次独立擦除：得到三段，四处切口。
    // 注意一次传两个点是“沿这两点连成的折线擦”，会把中间整段抹掉。
    d.erase_path(revision, &[Point { x: 10.0, y: 0.0 }], 2.0)
        .unwrap();
    let revision = d.state().revision;
    d.erase_path(revision, &[Point { x: 20.0, y: 0.0 }], 2.0)
        .unwrap();
    assert_eq!(d.state().stroke_count, 3);

    let counts: Vec<usize> = (0..3)
        .map(|index| {
            points_of(&d, index)
                .iter()
                .filter(|sample| sample.flags & FLAG_CUT_END != 0)
                .count()
        })
        .collect();
    // 首段与末段各一处切口，中间段两处。
    assert_eq!(counts, vec![1, 2, 1]);
}

#[test]
fn marks_survive_editing_and_snapshot_round_trip() {
    let mut d = cut_in_two();
    let revision = d.state().revision;

    // 变换、平移都保留标记。
    d.transform_strokes(
        revision,
        &[0, 1],
        &scratchmark_core::Transform::translate(5.0, 5.0),
    )
    .unwrap();
    assert_ne!(points_of(&d, 0)[1].flags & FLAG_CUT_END, 0);
    d.translate_all(d.state().revision, 0.0, 3.0).unwrap();
    assert_ne!(points_of(&d, 1)[0].flags & FLAG_CUT_END, 0);

    // 快照往返（格式 1，因为 flags 就在样本里，不需要新格式）。
    let bytes = d.save().unwrap();
    assert_eq!(&bytes[..8], b"SCRATCH\0");
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_ne!(points_of(&reopened, 0)[1].flags & FLAG_CUT_END, 0);
    assert_eq!(points_of(&reopened, 0)[0].flags & FLAG_CUT_END, 0);

    // 复制粘贴保留标记。
    let revision = reopened.state().revision;
    let blob = reopened.copy_strokes(revision, &[0]).unwrap();
    reopened
        .paste(reopened.state().revision, &blob, None)
        .unwrap();
    let last = reopened.state().stroke_count - 1;
    assert_ne!(points_of(&reopened, last)[1].flags & FLAG_CUT_END, 0);
}

#[test]
fn regularization_clears_the_cut_mark() {
    let mut d = cut_in_two();
    let revision = d.state().revision;
    d.regularize_stroke(revision, 0, ShapeKind::Line, false)
        .unwrap();
    for sample in points_of(&d, 0) {
        assert_eq!(sample.flags, 0, "规整生成的理想几何不应带任何标记");
    }
}

#[test]
fn write_channel_rejects_forged_cut_marks() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    for flags in [FLAG_CUT_END, FLAG_PRESSURE | FLAG_CUT_END] {
        assert_eq!(
            d.append(&[Sample {
                flags,
                pressure: if flags == 1 { 0.5 } else { 0.0 },
                ..point(0.0, 0.0, 0)
            }]),
            Err(Error::InvalidArgument)
        );
    }
    assert_eq!(d.state().active_points, 0);
    d.cancel().unwrap();
}

#[test]
fn loading_rejects_out_of_range_flags() {
    // 手工构造格式 1：一笔一点，flags = 4（越过位域）。
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SCRATCH\0");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&2.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0.0f64.to_le_bytes());
    bytes.extend_from_slice(&0.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0.0f32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());

    let mut d = Document::default();
    let revision = d.state().revision;
    assert_eq!(d.load(&bytes), Err(Error::InvalidData));
    assert_eq!(d.state().revision, revision);

    // bit0 未置位却带压力：同样拒绝。
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SCRATCH\0");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&2.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0.0f64.to_le_bytes());
    bytes.extend_from_slice(&0.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0.5f32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(d.load(&bytes), Err(Error::InvalidData));
}

#[test]
fn marks_do_not_affect_hit_testing() {
    let d = cut_in_two();
    // 切口处已经擦掉，命中不到；两段内部仍能命中。
    assert_eq!(d.hit_test(d.state().revision, 5.0, 0.0, 0.0).unwrap(), None);
    assert_eq!(
        d.hit_test(d.state().revision, 1.0, 0.0, 0.0).unwrap(),
        Some(0)
    );
    assert_eq!(
        d.hit_test(d.state().revision, 9.0, 0.0, 0.0).unwrap(),
        Some(1)
    );
}
