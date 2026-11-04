use scratchmark_core::{BrushOptions, Document, Error, Sample};

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn bytes_of(text: &str) -> Vec<u8> {
    // 夹具手工按规格编码，不调用被测编码器构造真值。
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}

fn golden_v1() -> Vec<u8> {
    bytes_of(include_str!("../../../spec/fixtures/simple-v1.hex"))
}

fn golden_v2_page42() -> Vec<u8> {
    bytes_of(include_str!("../../../spec/fixtures/simple-v2-page42.hex"))
}

// 两个夹具携带同一笔笔划：宽度 2、颜色 0x11223344、点 (0,1,t=10) 与 (2,-1,t=20,p=0.5,真实压力)。
fn fixture_samples() -> [Sample; 2] {
    [
        sample(0.0, 1.0, 10),
        Sample {
            x: 2.0,
            y: -1.0,
            time_us: 20,
            pressure: 0.5,
            flags: 1,
        },
    ]
}

#[test]
fn page_identity_rules_and_revision_tracking() {
    let mut d = Document::default();
    assert_eq!(d.page_id(), 0);
    let before = d.state().revision;
    assert_eq!(d.set_page_id(42), Ok(()));
    assert_eq!(d.page_id(), 42);
    assert_eq!(d.state().revision, before + 1);
    // 相同值成功且不递增；0 也允许写回（清除页面引用）。
    assert_eq!(d.set_page_id(42), Ok(()));
    assert_eq!(d.state().revision, before + 1);
    assert_eq!(d.set_page_id(0), Ok(()));
    assert_eq!(d.page_id(), 0);
    assert_eq!(d.state().revision, before + 2);
    // 活动笔划期间拒绝。
    d.begin(2.0, 0).unwrap();
    assert_eq!(d.set_page_id(7), Err(Error::InvalidState));
    assert_eq!(d.page_id(), 0);
    d.append(&[sample(1.0, 1.0, 0)]).unwrap();
    assert_eq!(d.set_page_id(7), Err(Error::InvalidState));
    d.cancel().unwrap();
    // 撤销 / 重做不回退页面标识。
    assert_eq!(d.set_page_id(7), Ok(()));
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(1.0, 1.0, 0)]).unwrap();
    d.finish().unwrap();
    d.undo().unwrap();
    assert_eq!(d.page_id(), 7);
    d.redo().unwrap();
    assert_eq!(d.page_id(), 7);
}

#[test]
fn format_two_snapshot_roundtrips_page_identity_and_samples() {
    let mut d = Document::default();
    assert_eq!(d.set_page_id(42), Ok(()));
    d.begin(2.0, 0x11223344).unwrap();
    d.append(&fixture_samples()).unwrap();
    d.finish().unwrap();
    let bytes = d.save().unwrap();
    assert_eq!(bytes, golden_v2_page42());
    assert_eq!(&bytes[..8], b"SCRATCH2");
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.page_id(), 42);
    let revision = reopened.state().revision;
    let mut raw = [sample(0.0, 0.0, 0); 2];
    reopened.read_points(revision, 0, 0, &mut raw).unwrap();
    assert_eq!(raw, fixture_samples());
    assert_eq!(reopened.save().unwrap(), bytes);
    // 再设置相同标识不改变快照字节。
    reopened.set_page_id(42).unwrap();
    assert_eq!(reopened.save().unwrap(), bytes);
}

#[test]
fn unset_page_identity_still_saves_format_one() {
    let mut d = Document::default();
    d.begin(2.0, 0x11223344).unwrap();
    d.append(&fixture_samples()).unwrap();
    d.finish().unwrap();
    assert_eq!(d.save().unwrap(), golden_v1());
    // 旧格式夹具读取后页面标识为 0，继续写回格式 1。
    let mut loaded = Document::default();
    loaded.load(&golden_v1()).unwrap();
    assert_eq!(loaded.page_id(), 0);
    assert_eq!(loaded.save().unwrap(), golden_v1());
}

#[test]
fn cross_page_load_is_rejected_atomically_and_zero_clears_identity() {
    let mut d = Document::default();
    d.set_page_id(99).unwrap();
    d.begin(3.0, 0).unwrap();
    d.append(&[sample(5.0, 5.0, 0)]).unwrap();
    d.finish().unwrap();
    let before = d.save().unwrap();
    let revision = d.state().revision;
    assert_eq!(d.load(&golden_v2_page42()), Err(Error::InvalidData));
    assert_eq!(d.page_id(), 99);
    assert_eq!(d.save().unwrap(), before);
    assert_eq!(d.state().revision, revision);
    // 快照标识为 0 按整份替换语义清空当前标识。
    d.load(&golden_v1()).unwrap();
    assert_eq!(d.page_id(), 0);
    assert_eq!(d.state().stroke_count, 1);
    // 两个非零标识相同则允许重复恢复。
    let mut same = Document::default();
    same.set_page_id(42).unwrap();
    assert_eq!(same.load(&golden_v2_page42()), Ok(()));
    assert_eq!(same.page_id(), 42);
    // 格式 2 也可以携带 0，读取后同样清空。
    let mut bytes = golden_v2_page42();
    bytes[16..24].fill(0);
    let mut fresh = Document::default();
    fresh.set_page_id(1).unwrap();
    fresh.load(&bytes).unwrap();
    assert_eq!(fresh.page_id(), 0);
}

#[test]
fn empty_page_with_identity_saves_and_restores() {
    let mut d = Document::default();
    d.set_page_id(0xdead_beef).unwrap();
    let bytes = d.save().unwrap();
    let mut expected = Vec::new();
    expected.extend_from_slice(b"SCRATCH2");
    expected.extend_from_slice(&2u32.to_le_bytes());
    expected.extend_from_slice(&0u32.to_le_bytes());
    expected.extend_from_slice(&0xdead_beef_u64.to_le_bytes());
    assert_eq!(bytes, expected);
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.page_id(), 0xdead_beef);
    assert_eq!(reopened.state().stroke_count, 0);
    assert_eq!(reopened.save().unwrap(), bytes);
    // 空页面同样可以继续书写。
    reopened.begin(2.0, 0).unwrap();
    reopened.append(&[sample(1.0, 1.0, 0)]).unwrap();
    reopened.finish().unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
    assert_eq!(reopened.page_id(), 0xdead_beef);
}

#[test]
fn format_two_rejects_malformed_headers_and_strokes_like_format_one() {
    let mut d = Document::default();
    d.set_page_id(1).unwrap();
    let valid = golden_v2_page42();
    let revision = d.state().revision;
    for length in 0..valid.len() {
        assert!(d.load(&valid[..length]).is_err());
        assert_eq!(d.state().revision, revision);
    }
    let mut cases = Vec::new();
    let mut bad_version = valid.clone();
    bad_version[8] = 3;
    cases.push((bad_version, Error::UnsupportedVersion));
    let mut trailing = valid.clone();
    trailing.push(0);
    cases.push((trailing, Error::InvalidData));
    // 旧魔数即使声明版本 1 也不接受格式 2 的多余页面标识字段：
    // 页面标识字节被当作笔宽解析后字段整体错位，错误码随错位结果而定。
    let mut wrong_magic = valid.clone();
    wrong_magic[..8].copy_from_slice(b"SCRATCH\0");
    wrong_magic[8] = 1;
    cases.push((wrong_magic, Error::LimitExceeded));
    let mut unknown_magic = valid.clone();
    unknown_magic[..8].copy_from_slice(b"SCRATCH3");
    cases.push((unknown_magic, Error::InvalidData));
    let mut bad_stroke = valid.clone();
    bad_stroke[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
    cases.push((bad_stroke, Error::InvalidData));
    let mut empty_stroke = valid.clone();
    empty_stroke[36..40].fill(0);
    cases.push((empty_stroke, Error::InvalidData));
    for (bytes, expected) in cases {
        assert_eq!(d.load(&bytes), Err(expected));
        assert_eq!(d.page_id(), 1);
        assert_eq!(d.state().revision, revision);
    }
}

#[test]
fn brush_reads_are_identical_across_formats_for_the_same_stroke() {
    let options = BrushOptions {
        alpha: 0.5,
        min_width_ratio: 0.25,
        pressure_enabled: 1,
        reserved: 0,
    };
    let mut from_v1 = Document::default();
    from_v1.load(&golden_v1()).unwrap();
    let mut from_v2 = Document::default();
    from_v2.load(&golden_v2_page42()).unwrap();
    let mut points_v1 = [scratchmark_core::BrushPoint::default(); 2];
    let mut points_v2 = [scratchmark_core::BrushPoint::default(); 2];
    let revision = from_v1.state().revision;
    from_v1
        .read_brush_points(revision, 0, 0, options, &mut points_v1)
        .unwrap();
    from_v2
        .read_brush_points(from_v2.state().revision, 0, 0, options, &mut points_v2)
        .unwrap();
    assert_eq!(points_v1, points_v2);
}
