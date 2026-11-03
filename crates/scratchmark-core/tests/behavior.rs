use scratchmark_core::{Document, Error, MAX_BATCH, MAX_POINTS, Sample};

fn point(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn commit(d: &mut Document, x: f64) {
    d.begin(2.0, 0x11223344).unwrap();
    d.append(&[point(x, 0.0, 0)]).unwrap();
    d.finish().unwrap();
}

fn golden() -> Vec<u8> {
    // 夹具手工按规格编码，不调用被测编码器构造真值。
    include_str!("../../../spec/fixtures/simple-v1.hex")
        .split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}

#[test]
fn active_stroke_is_visible_and_reads_are_revision_bound() {
    let mut d = Document::default();
    let old = d.state().revision;
    d.begin(2.0, 0x11223344).unwrap();
    assert_eq!(d.stroke_info(old, 0).unwrap_err(), Error::StaleRevision);
    let empty = d.stroke_info(d.state().revision, 0).unwrap();
    assert_eq!((empty.active, empty.point_count, empty.min_x), (1, 0, 0.0));
    d.append(&[point(0.0, 1.0, 10), point(2.0, -1.0, 20)])
        .unwrap();
    let info = d.stroke_info(d.state().revision, 0).unwrap();
    assert_eq!(
        (info.min_x, info.min_y, info.max_x, info.max_y),
        (-1.0, -2.0, 3.0, 2.0)
    );
    let mut result = [point(99.0, 99.0, 0); 3];
    assert_eq!(
        d.read_points(d.state().revision, 0, 1, &mut result)
            .unwrap(),
        1
    );
    assert_eq!(result[0], point(2.0, -1.0, 20));
    assert_eq!(result[1].x, 99.0);
    d.finish().unwrap();
    assert_eq!(d.stroke_info(d.state().revision, 0).unwrap().active, 0);
}

#[test]
fn invalid_batch_is_atomic_even_after_a_valid_prefix() {
    let mut d = Document::default();
    d.begin(2.0, 0).unwrap();
    d.append(&[point(1.0, 1.0, 10)]).unwrap();
    let before = d.state().revision;
    for invalid in [
        point(f64::NAN, 0.0, 20),
        point(0.0, 0.0, 9),
        Sample {
            pressure: 0.5,
            ..point(0.0, 0.0, 20)
        },
        Sample {
            flags: 2,
            ..point(0.0, 0.0, 20)
        },
    ] {
        assert_eq!(
            d.append(&[point(2.0, 2.0, 11), invalid]),
            Err(Error::InvalidArgument)
        );
        assert_eq!(d.state().active_points, 1);
        assert_eq!(d.state().revision, before);
    }
    d.append(&[]).unwrap();
    assert_eq!(d.state().revision, before);
}

#[test]
fn cancellation_preserves_redo_but_a_commit_replaces_it() {
    let mut d = Document::default();
    commit(&mut d, 1.0);
    d.undo().unwrap();
    d.begin(1.0, 0).unwrap();
    d.append(&[point(9.0, 9.0, 0)]).unwrap();
    assert_eq!(d.undo(), Err(Error::InvalidState));
    d.cancel().unwrap();
    d.redo().unwrap();
    assert_eq!(d.state().stroke_count, 1);
    d.undo().unwrap();
    commit(&mut d, 2.0);
    assert_eq!(d.redo(), Err(Error::InvalidState));
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn empty_or_illegal_operations_do_not_change_state() {
    let mut d = Document::default();
    for width in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert_eq!(d.begin(width, 0), Err(Error::InvalidArgument));
        assert_eq!(d.state().revision, 1);
    }
    assert_eq!(d.append(&[]), Err(Error::InvalidState));
    assert_eq!(d.cancel(), Err(Error::InvalidState));
    assert_eq!(d.undo(), Err(Error::InvalidState));
    d.begin(1.0, 0).unwrap();
    let r = d.state().revision;
    assert_eq!(d.finish(), Err(Error::InvalidState));
    assert_eq!(d.begin(2.0, 0), Err(Error::InvalidState));
    assert_eq!(d.save(), Err(Error::InvalidState));
    assert_eq!(d.load(&golden()), Err(Error::InvalidState));
    assert_eq!(d.state().revision, r);
}

#[test]
fn independent_fixture_decodes_and_encodes_exactly() {
    let mut d = Document::default();
    d.load(&golden()).unwrap();
    let r = d.state().revision;
    let info = d.stroke_info(r, 0).unwrap();
    assert_eq!(
        (info.width, info.color, info.point_count),
        (2.0, 0x11223344, 2)
    );
    let mut points = [point(0.0, 0.0, 0); 2];
    d.read_points(r, 0, 0, &mut points).unwrap();
    assert_eq!(points[0], point(0.0, 1.0, 10));
    assert_eq!(
        points[1],
        Sample {
            pressure: 0.5,
            flags: 1,
            ..point(2.0, -1.0, 20)
        }
    );
    assert_eq!(d.save().unwrap(), golden());
}

#[test]
fn save_reopen_and_continue_editing_without_restoring_history() {
    let mut first = Document::default();
    commit(&mut first, 1.0);
    commit(&mut first, 2.0);
    first.undo().unwrap();
    let bytes = first.save().unwrap();
    let mut reopened = Document::default();
    reopened.load(&bytes).unwrap();
    assert_eq!(reopened.state().stroke_count, 1);
    assert_eq!(reopened.redo(), Err(Error::InvalidState));
    commit(&mut reopened, 3.0);
    assert_eq!(reopened.state().stroke_count, 2);
    reopened.undo().unwrap();
    reopened.redo().unwrap();
    assert_eq!(reopened.state().stroke_count, 2);
}

#[test]
fn malformed_snapshot_never_replaces_a_live_document() {
    let mut d = Document::default();
    commit(&mut d, 8.0);
    let before = d.save().unwrap();
    let r = d.state().revision;
    let valid = golden();
    for length in 0..valid.len() {
        assert!(d.load(&valid[..length]).is_err());
        assert_eq!(d.state().revision, r);
    }
    let mut cases = Vec::new();
    let mut version = valid.clone();
    version[8] = 2;
    cases.push((version, Error::UnsupportedVersion));
    let mut trailing = valid.clone();
    trailing.push(0);
    cases.push((trailing, Error::InvalidData));
    let mut excessive = valid.clone();
    excessive[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push((excessive, Error::LimitExceeded));
    let mut nan = valid.clone();
    nan[32..40].copy_from_slice(&f64::NAN.to_le_bytes());
    cases.push((nan, Error::InvalidData));
    let mut empty = valid.clone();
    empty[28..32].fill(0);
    cases.push((empty, Error::InvalidData));
    let mut reversed = valid.clone();
    reversed[80..88].copy_from_slice(&9u64.to_le_bytes());
    cases.push((reversed, Error::InvalidData));
    for (bytes, expected) in cases {
        assert_eq!(d.load(&bytes), Err(expected));
        assert_eq!(d.save().unwrap(), before);
        assert_eq!(d.state().revision, r);
    }
}

#[test]
fn resource_limits_reject_without_partial_append_and_cancel_releases_budget() {
    let mut d = Document::default();
    d.begin(1.0, 0).unwrap();
    let batch = vec![point(0.0, 0.0, 0); MAX_BATCH];
    assert_eq!(
        d.append(&vec![batch[0]; MAX_BATCH + 1]),
        Err(Error::LimitExceeded)
    );
    let mut count = 0;
    while count < MAX_POINTS {
        let n = MAX_BATCH.min(MAX_POINTS - count);
        d.append(&batch[..n]).unwrap();
        count += n;
    }
    let r = d.state().revision;
    assert_eq!(d.append(&batch[..1]), Err(Error::LimitExceeded));
    assert_eq!(d.state().revision, r);
    assert_eq!(d.state().active_points as usize, MAX_POINTS);
    d.cancel().unwrap();
    commit(&mut d, 1.0);
    assert_eq!(d.state().stroke_count, 1);
}

#[test]
fn empty_snapshot_and_declared_stroke_limit_are_checked_without_allocation() {
    let empty = [83, 67, 82, 65, 84, 67, 72, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    let mut d = Document::default();
    assert_eq!(d.save().unwrap(), empty);
    d.load(&empty).unwrap();
    assert_eq!(d.state().stroke_count, 0);
    let mut huge = empty;
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(d.load(&huge), Err(Error::LimitExceeded));
    assert_eq!(d.save().unwrap(), empty);
}

#[test]
fn point_dot_bounds_and_invalid_read_preserve_the_destination() {
    let mut d = Document::default();
    d.begin(4.0, 0).unwrap();
    d.append(&[Sample {
        pressure: 1.0,
        flags: 1,
        ..point(3.0, 5.0, 0)
    }])
    .unwrap();
    let r = d.state().revision;
    let b = d.stroke_info(r, 0).unwrap();
    assert_eq!((b.min_x, b.min_y, b.max_x, b.max_y), (1.0, 3.0, 5.0, 7.0));
    let sentinel = point(99.0, 99.0, 0);
    let mut output = [sentinel];
    assert_eq!(
        d.read_points(r, 0, 2, &mut output),
        Err(Error::InvalidArgument)
    );
    assert_eq!(output, [sentinel]);
    assert_eq!(d.read_points(r, 0, 1, &mut output).unwrap(), 0);
    assert_eq!(d.stroke_info(r, 1).unwrap_err(), Error::InvalidArgument);
}
