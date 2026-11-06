use scratchmark_core::{Document, Error, MAX_COORD, Point, Sample, Transform};

fn point(x: f64, y: f64, time_us: u64, pressure: f32, flags: u32) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure,
        flags,
    }
}

fn line(d: &mut Document, from: (f64, f64), to: (f64, f64), width: f64) {
    d.begin(width, 0x11223344).unwrap();
    d.append(&[
        point(from.0, from.1, 0, 0.0, 0),
        point(to.0, to.1, 1000, 0.5, 1),
    ])
    .unwrap();
    d.finish().unwrap();
}

fn stroke(d: &Document, index: u32) -> (StrokeFields, Vec<Sample>) {
    let revision = d.state().revision;
    let info = d.stroke_info(revision, index).unwrap();
    let mut buffer = vec![point(0.0, 0.0, 0, 0.0, 0); info.point_count as usize];
    let written = d.read_points(revision, index, 0, &mut buffer).unwrap();
    buffer.truncate(written);
    (
        StrokeFields {
            width: info.width,
            color: info.color,
            min_x: info.min_x,
            max_x: info.max_x,
        },
        buffer,
    )
}

struct StrokeFields {
    width: f64,
    color: u32,
    min_x: f64,
    #[allow(dead_code)]
    max_x: f64,
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn translate_keeps_width_and_sample_metadata() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let revision = d.state().revision;

    d.transform_strokes(revision, &[0], &Transform::translate(10.0, -5.0))
        .unwrap();
    let (fields, points) = stroke(&d, 0);
    assert_eq!(fields.width, 2.0);
    assert_eq!(fields.color, 0x11223344);
    assert_eq!(points.len(), 2);
    close(points[0].x, 10.0);
    close(points[0].y, -5.0);
    close(points[1].x, 20.0);
    close(points[1].y, -5.0);
    // 时间戳、压力与 flags 不因几何变换而改变。
    assert_eq!(points[1].time_us, 1000);
    assert_eq!(points[1].pressure, 0.5);
    assert_eq!(points[1].flags, 1);

    d.undo().unwrap();
    let (fields, points) = stroke(&d, 0);
    assert_eq!(fields.width, 2.0);
    close(points[0].x, 0.0);
    d.redo().unwrap();
    close(stroke(&d, 0).1[0].x, 10.0);
}

#[test]
fn scale_rotation_and_mirror_follow_hand_computed_values() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);

    // 非等比缩放：线宽取等面积因子 √|det| = √6。
    d.transform_strokes(d.state().revision, &[0], &Transform::scale(2.0, 3.0))
        .unwrap();
    let (fields, points) = stroke(&d, 0);
    close(fields.width, 2.0 * 6f64.sqrt());
    close(points[1].x, 20.0);
    close(points[1].y, 0.0);
    d.undo().unwrap();

    // 90° 旋转：(x, y) -> (y, -x)，det = 1，线宽不变。
    let rotate = Transform {
        m00: 0.0,
        m01: 1.0,
        m10: -1.0,
        m11: 0.0,
        dx: 0.0,
        dy: 0.0,
    };
    d.transform_strokes(d.state().revision, &[0], &rotate)
        .unwrap();
    let (fields, points) = stroke(&d, 0);
    assert_eq!(fields.width, 2.0);
    close(points[1].x, 0.0);
    close(points[1].y, -10.0);
    d.undo().unwrap();

    // 镜像：det = -1，线宽仍不变，坐标取反。
    let mirror = Transform {
        m00: -1.0,
        m01: 0.0,
        m10: 0.0,
        m11: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
    d.transform_strokes(d.state().revision, &[0], &mirror)
        .unwrap();
    let (fields, points) = stroke(&d, 0);
    assert_eq!(fields.width, 2.0);
    close(points[1].x, -10.0);
}

#[test]
fn invalid_transforms_are_rejected_without_changing_document() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let state = d.state();
    let bad = [
        // det == 0
        Transform {
            m00: 1.0,
            m01: 0.0,
            m10: 0.0,
            m11: 0.0,
            dx: 0.0,
            dy: 0.0,
        },
        // 非有限系数
        Transform {
            m00: f64::NAN,
            ..Transform::IDENTITY
        },
        Transform {
            dy: f64::INFINITY,
            ..Transform::IDENTITY
        },
        // |det| 过小 / 过大
        Transform::scale(1e-7, 1e-7),
        Transform::scale(1e7, 1e7),
    ];
    for transform in bad {
        assert_eq!(
            d.transform_strokes(state.revision, &[0], &transform),
            Err(Error::InvalidArgument)
        );
        assert_eq!(d.state(), state);
    }
}

#[test]
fn out_of_page_and_width_overflow_fail_atomically() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let state = d.state();

    // 平移到页面域外：x' 超过 1e9。
    assert_eq!(
        d.transform_strokes(state.revision, &[0], &Transform::translate(MAX_COORD, 0.0)),
        Err(Error::InvalidArgument)
    );
    // det == 1e12 通过矩阵校验，但线宽 2×1e6 超过 1e6 上限。
    assert_eq!(
        d.transform_strokes(state.revision, &[0], &Transform::scale(1e6, 1e6)),
        Err(Error::InvalidArgument)
    );
    assert_eq!(d.state(), state);
    close(stroke(&d, 0).1[1].x, 10.0);
}

#[test]
fn transform_requires_idle_matching_revision_and_valid_indices() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    let state = d.state();
    let move_right = Transform::translate(5.0, 0.0);

    assert_eq!(
        d.transform_strokes(state.revision + 1, &[0], &move_right),
        Err(Error::StaleRevision)
    );
    assert_eq!(
        d.transform_strokes(state.revision, &[0, 0], &move_right),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.transform_strokes(state.revision, &[2], &move_right),
        Err(Error::InvalidArgument)
    );
    assert_eq!(d.state(), state);

    // 空选区是空操作。
    d.transform_strokes(state.revision, &[], &move_right)
        .unwrap();
    assert_eq!(d.state().revision, state.revision);

    d.begin(2.0, 0).unwrap();
    let active = d.state().revision;
    assert_eq!(
        d.transform_strokes(active, &[0], &move_right),
        Err(Error::InvalidState)
    );
    d.cancel().unwrap();
    assert_eq!(d.state().stroke_count, 2);
}

#[test]
fn copy_exports_selection_as_format1_snapshot() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    line(&mut d, (200.0, 0.0), (210.0, 0.0), 2.0);
    let state = d.state();

    // 索引顺序任意，输出按索引升序。
    let blob = d.copy_strokes(state.revision, &[2, 0]).unwrap();
    assert_eq!(&blob[..8], b"SCRATCH\0");
    assert_eq!(d.state(), state, "复制是只读操作");

    let mut restored = Document::default();
    restored.load(&blob).unwrap();
    assert_eq!(restored.state().stroke_count, 2);
    close(stroke(&restored, 0).1[0].x, 0.0);
    close(stroke(&restored, 1).1[0].x, 200.0);

    // 空选区产出 0 笔的格式 1 快照，可以正常载入。
    let empty = d.copy_strokes(state.revision, &[]).unwrap();
    let mut nothing = Document::default();
    nothing.load(&empty).unwrap();
    assert_eq!(nothing.state().stroke_count, 0);

    assert_eq!(
        d.copy_strokes(state.revision - 1, &[0]),
        Err(Error::StaleRevision)
    );
    assert_eq!(
        d.copy_strokes(state.revision, &[0, 0]),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        d.copy_strokes(state.revision, &[9]),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn paste_appends_on_top_and_is_a_single_undo() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut d, (100.0, 0.0), (110.0, 0.0), 2.0);
    let before = d.state();
    let blob = d.copy_strokes(before.revision, &[1]).unwrap();

    d.paste(before.revision, &blob, None).unwrap();
    let after = d.state();
    assert_eq!(after.stroke_count, 3);
    assert_eq!(after.can_undo, 1);
    assert_eq!(after.can_redo, 0);
    // 粘贴进来的笔划在最上层，几何与源笔划一致。
    close(stroke(&d, 2).1[0].x, 100.0);

    d.undo().unwrap();
    assert_eq!(d.state().stroke_count, 2);
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 3);
}

#[test]
fn paste_with_transform_moves_the_batch() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let revision = d.state().revision;
    let blob = d.copy_strokes(revision, &[0]).unwrap();

    d.paste(revision, &blob, Some(&Transform::translate(100.0, 50.0)))
        .unwrap();
    assert_eq!(d.state().stroke_count, 2);
    let (fields, points) = stroke(&d, 1);
    assert_eq!(fields.width, 2.0);
    close(points[0].x, 100.0);
    close(points[0].y, 50.0);
    close(fields.min_x, 99.0);
}

#[test]
fn paste_rejects_format2_garbage_and_empty_documents() {
    let mut source = Document::default();
    source.set_page_id(9).unwrap();
    line(&mut source, (0.0, 0.0), (10.0, 0.0), 2.0);
    let format2 = source.save().unwrap();
    assert_eq!(&format2[..8], b"SCRATCH2");

    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    let state = d.state();

    // 格式 2 是“恢复文档”的字节，不是剪贴板内容。
    assert_eq!(
        d.paste(state.revision, &format2, None),
        Err(Error::InvalidData)
    );
    assert_eq!(
        d.paste(state.revision, &[0u8; 4], None),
        Err(Error::InvalidData)
    );
    let mut unknown = format2.clone();
    unknown[0] = b'X';
    assert_eq!(
        d.paste(state.revision, &unknown, None),
        Err(Error::InvalidData)
    );
    assert_eq!(d.state(), state);

    // 未知版本沿用既有分类。
    let mut wrong_version = vec![];
    wrong_version.extend_from_slice(b"SCRATCH\0");
    wrong_version.extend_from_slice(&7u32.to_le_bytes());
    assert_eq!(
        d.paste(state.revision, &wrong_version, None),
        Err(Error::UnsupportedVersion)
    );

    // 截断：只保留头与前一笔的前缀。
    let blob = d.copy_strokes(state.revision, &[0]).unwrap();
    let truncated = &blob[..blob.len() - 8];
    assert_eq!(
        d.paste(state.revision, truncated, None),
        Err(Error::InvalidData)
    );

    // 空快照是空操作：不改 revision。
    let empty = d.copy_strokes(state.revision, &[]).unwrap();
    d.paste(state.revision, &empty, None).unwrap();
    assert_eq!(d.state().revision, state.revision);
}

#[test]
fn paste_works_across_documents_and_clears_redo() {
    let mut source = Document::default();
    line(&mut source, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(&mut source, (0.0, 10.0), (10.0, 10.0), 2.0);
    let blob = source
        .copy_strokes(source.state().revision, &[0, 1])
        .unwrap();

    let mut target = Document::default();
    line(&mut target, (500.0, 500.0), (510.0, 500.0), 2.0);
    target.undo().unwrap();
    assert_eq!(target.state().can_redo, 1);

    target.paste(target.state().revision, &blob, None).unwrap();
    assert_eq!(target.state().stroke_count, 2);
    assert_eq!(target.state().can_redo, 0);
    close(stroke(&target, 0).1[0].x, 0.0);
    close(stroke(&target, 1).1[0].y, 10.0);

    // 粘贴后仍可整笔撤销与删除。
    let revision = target.state().revision;
    let hits = target
        .lasso_hits(
            revision,
            &[
                Point { x: -5.0, y: -5.0 },
                Point { x: 15.0, y: -5.0 },
                Point { x: 15.0, y: 15.0 },
                Point { x: -5.0, y: 15.0 },
            ],
        )
        .unwrap();
    assert_eq!(hits, vec![0, 1]);
}

#[test]
fn transformed_selection_survives_snapshot_round_trip() {
    let mut d = Document::default();
    line(&mut d, (0.0, 0.0), (10.0, 0.0), 2.0);
    d.transform_strokes(d.state().revision, &[0], &Transform::translate(30.0, 40.0))
        .unwrap();
    let bytes = d.save().unwrap();
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    let (fields, points) = stroke(&reopened, 0);
    close(fields.min_x, 29.0);
    close(points[0].y, 40.0);
}
