use scratchmark_core::{Document, Error, Point, Sample, SampleAxes, ShapeKind, Transform};

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn axes(tilt: f32, orientation: f32) -> SampleAxes {
    SampleAxes { tilt, orientation }
}

/// 一笔两点，轴向分别给定。
fn commit_with_axes(d: &mut Document, first: SampleAxes, second: SampleAxes) {
    d.begin(2.0, 0x11223344).unwrap();
    let samples = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 1000)];
    d.append_with_axes(&samples, Some(&[first, second]))
        .unwrap();
    d.finish().unwrap();
}

fn read_axes_of(d: &Document, index: u32) -> Vec<SampleAxes> {
    let revision = d.state().revision;
    let info = d.stroke_info(revision, index).unwrap();
    let mut buffer = vec![SampleAxes::default(); info.point_count as usize];
    let written = d.read_axes(revision, index, 0, &mut buffer).unwrap();
    buffer.truncate(written);
    buffer
}

#[test]
fn append_and_read_axes_round_trip() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    let samples = [
        sample(0.0, 0.0, 0),
        sample(5.0, 0.0, 500),
        sample(10.0, 0.0, 1000),
    ];
    let values = [axes(0.0, 0.0), axes(0.5, -1.2), axes(1.4, 3.0)];
    d.append_with_axes(&samples, Some(&values)).unwrap();
    let revision = d.state().revision;

    // 活动笔划（索引 = stroke_count）也能读。
    let mut buffer = [SampleAxes::default(); 3];
    assert_eq!(d.read_axes(revision, 0, 0, &mut buffer).unwrap(), 3);
    assert_eq!(buffer, values);

    // 分段读取与越界。
    let mut one = [SampleAxes::default(); 1];
    assert_eq!(d.read_axes(revision, 0, 2, &mut one).unwrap(), 1);
    assert_eq!(one[0], values[2]);
    assert_eq!(d.read_axes(revision, 0, 3, &mut one), Ok(0));
    assert_eq!(
        d.read_axes(revision, 0, 4, &mut one),
        Err(Error::InvalidArgument)
    );
    d.cancel().unwrap();

    // 不提供轴向时按默认值补齐，通道仍然等长。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(0.0, 0.0, 0), sample(1.0, 0.0, 1)])
        .unwrap();
    d.finish().unwrap();
    assert_eq!(
        read_axes_of(&d, 0),
        vec![SampleAxes::default(), SampleAxes::default()]
    );
}

#[test]
fn invalid_axes_are_rejected() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    let samples = [sample(0.0, 0.0, 0), sample(1.0, 0.0, 1)];

    // 长度不一致。
    assert_eq!(
        d.append_with_axes(&samples, Some(&[axes(0.0, 0.0)])),
        Err(Error::InvalidArgument)
    );
    // 倾斜越界（> π/2）、负值、非有限。
    for bad in [
        axes(2.0, 0.0),
        axes(-0.1, 0.0),
        axes(f32::NAN, 0.0),
        axes(0.0, f32::INFINITY),
        axes(0.0, 4.0),
    ] {
        assert_eq!(
            d.append_with_axes(&samples, Some(&[bad, axes(0.0, 0.0)])),
            Err(Error::InvalidArgument)
        );
    }
    // 失败不改变活动笔划。
    assert_eq!(d.state().active_points, 0);
    d.cancel().unwrap();
}

#[test]
fn snapshot_uses_format_three_only_when_axes_are_present() {
    // 无轴向：仍是格式 1。
    let mut plain = Document::default();
    plain.begin(2.0, 0).unwrap();
    plain.append(&[sample(0.0, 0.0, 0)]).unwrap();
    plain.finish().unwrap();
    assert_eq!(&plain.save().unwrap()[..8], b"SCRATCH\0");

    // 有页面标识、无轴向：格式 2。
    let mut paged = Document::default();
    paged.set_page_id(42).unwrap();
    paged.begin(2.0, 0).unwrap();
    paged.append(&[sample(0.0, 0.0, 0)]).unwrap();
    paged.finish().unwrap();
    assert_eq!(&paged.save().unwrap()[..8], b"SCRATCH2");

    // 有轴向：格式 3，且往返保持轴向。
    let mut axed = Document::default();
    axed.set_page_id(7).unwrap();
    commit_with_axes(&mut axed, axes(0.3, 1.0), axes(0.9, -2.5));
    let bytes = axed.save().unwrap();
    assert_eq!(&bytes[..8], b"SCRATCH3");

    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.page_id(), 7);
    assert_eq!(
        read_axes_of(&reopened, 0),
        vec![axes(0.3, 1.0), axes(0.9, -2.5)]
    );
    // 点与时间不受轴向通道影响。
    let revision = reopened.state().revision;
    let mut points = [sample(0.0, 0.0, 0); 2];
    assert_eq!(
        reopened.read_points(revision, 0, 0, &mut points).unwrap(),
        2
    );
    assert_eq!(points[1].x, 10.0);
    assert_eq!(points[1].time_us, 1000);
}

#[test]
fn older_formats_still_load_with_default_axes() {
    // 手工构造格式 1：魔数 + 版本 1 + 一笔 1 点。
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SCRATCH\0");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&2.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&5.0f64.to_le_bytes());
    bytes.extend_from_slice(&6.0f64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0.0f32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());

    let mut d = Document::default();
    d.load(&bytes).unwrap();
    assert_eq!(d.state().stroke_count, 1);
    assert_eq!(read_axes_of(&d, 0), vec![SampleAxes::default()]);
    // 读回来再存仍是格式 1（轴向仍为默认）。
    assert_eq!(&d.save().unwrap()[..8], b"SCRATCH\0");
}

#[test]
fn erase_path_fragments_keep_and_interpolate_axes() {
    let mut d = Document::default();
    commit_with_axes(&mut d, axes(0.0, 0.0), axes(1.0, 2.0));
    let revision = d.state().revision;
    // 在正中切掉半径 2：两段各保留起点/终点轴向。
    d.erase_path(revision, &[Point { x: 5.0, y: 0.0 }], 2.0)
        .unwrap();
    assert_eq!(d.state().stroke_count, 2);

    let first = read_axes_of(&d, 0);
    assert_eq!(first[0], axes(0.0, 0.0));
    // 切点在 t=0.3：轴向按两侧线性插值。
    let cut = first[1];
    assert!((cut.tilt - 0.3).abs() < 1e-6, "tilt {}", cut.tilt);
    assert!(
        (cut.orientation - 0.6).abs() < 1e-6,
        "orientation {}",
        cut.orientation
    );

    let second = read_axes_of(&d, 1);
    assert_eq!(second[1], axes(1.0, 2.0));
    assert!((second[0].tilt - 0.7).abs() < 1e-6);
}

#[test]
fn transform_keeps_axes_and_shape_regularization_resets_them() {
    let mut d = Document::default();
    commit_with_axes(&mut d, axes(0.25, 0.5), axes(0.75, -1.5));
    let revision = d.state().revision;
    d.transform_strokes(revision, &[0], &Transform::translate(10.0, 5.0))
        .unwrap();
    assert_eq!(read_axes_of(&d, 0), vec![axes(0.25, 0.5), axes(0.75, -1.5)]);

    // 规整生成的是理想几何，不带原手写的轴向。
    let revision = d.state().revision;
    d.regularize_stroke(revision, 0, ShapeKind::Line, false)
        .unwrap();
    assert_eq!(
        read_axes_of(&d, 0),
        vec![SampleAxes::default(), SampleAxes::default()]
    );
}

#[test]
fn copy_and_paste_carry_axes_through_format_three() {
    let mut d = Document::default();
    commit_with_axes(&mut d, axes(0.4, 1.1), axes(0.8, -0.6));
    let revision = d.state().revision;

    let blob = d.copy_strokes(revision, &[0]).unwrap();
    // 带轴向的复制产出格式 3，页面标识为 0。
    assert_eq!(&blob[..8], b"SCRATCH3");
    // 头部布局：magic(8) + 版本(4) + 笔数(4) + 页面标识(8)。
    assert_eq!(&blob[16..24], &0u64.to_le_bytes());

    d.paste(revision, &blob, None).unwrap();
    assert_eq!(d.state().stroke_count, 2);
    assert_eq!(read_axes_of(&d, 1), vec![axes(0.4, 1.1), axes(0.8, -0.6)]);
}

#[test]
fn paste_rejects_page_scoped_format_three_blobs() {
    let mut source = Document::default();
    source.set_page_id(9).unwrap();
    commit_with_axes(&mut source, axes(0.1, 0.2), axes(0.3, 0.4));
    let document = source.save().unwrap();
    assert_eq!(&document[..8], b"SCRATCH3");

    let mut target = Document::default();
    let revision = target.state().revision;
    // 带页面标识的格式 3 是“整份文档”，不是剪贴板内容。
    assert_eq!(
        target.paste(revision, &document, None),
        Err(Error::InvalidData)
    );
    assert_eq!(target.state().stroke_count, 0);
    assert_eq!(target.state().revision, revision);
}
