use super::*;
use crate::{sm_begin, sm_destroy, sm_features, sm_finish};
use std::ptr;

struct Handle(u64);
impl Handle {
    fn new() -> Self {
        let mut h = 0;
        assert_eq!(unsafe { sm_create(&mut h) }, 0);
        Self(h)
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        sm_destroy(self.0);
    }
}

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn cut_in_two(h: u64) -> u64 {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 100)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    let path = [Point { x: 5.0, y: 0.0 }];
    assert_eq!(
        unsafe { sm_erase_path(h, state.revision, path.as_ptr(), 1, 2.0) },
        0
    );
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state.revision
}

fn read(h: u64, revision: u64, index: u32) -> Vec<Sample> {
    let mut info = StrokeInfo::default();
    assert_eq!(unsafe { sm_get_stroke(h, revision, index, &mut info) }, 0);
    let mut buffer = vec![sample(0.0, 0.0, 0); info.point_count as usize];
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_read_points(
                h,
                revision,
                index,
                0,
                buffer.as_mut_ptr(),
                info.point_count,
                &mut written,
            )
        },
        0
    );
    buffer.truncate(written as usize);
    buffer
}

#[test]
fn features_report_cut_mark() {
    assert_eq!(sm_features() & 32768, 32768);
}

#[test]
fn erase_path_marks_only_the_cut_ends() {
    let h = Handle::new();
    let revision = cut_in_two(h.0);

    let first = read(h.0, revision, 0);
    assert_eq!(first[0].flags & 2, 0, "原始首点不带切点标记");
    assert_eq!(first[1].flags & 2, 2, "切出来的端点带标记");

    let second = read(h.0, revision, 1);
    assert_eq!(second[0].flags & 2, 2);
    assert_eq!(second[1].flags & 2, 0);
}

#[test]
fn marks_survive_load() {
    let h = Handle::new();
    cut_in_two(h.0);
    let mut required = 0;
    assert_eq!(
        unsafe { sm_save(h.0, ptr::null_mut(), 0, &mut required) },
        4
    );
    let mut bytes = vec![0u8; required as usize];
    assert_eq!(
        unsafe { sm_save(h.0, bytes.as_mut_ptr(), required, &mut required) },
        0
    );
    assert_eq!(&bytes[..8], b"SCRATCH\0", "切点标记不需要新快照格式");

    let other = Handle::new();
    assert_eq!(
        unsafe { sm_load(other.0, bytes.as_ptr(), bytes.len() as u32) },
        0
    );
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(other.0, &mut state) }, 0);
    let first = read(other.0, state.revision, 0);
    assert_eq!(first[1].flags & 2, 2);
}

#[test]
fn write_channel_rejects_forged_marks() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let forged = [Sample {
        x: 0.0,
        y: 0.0,
        time_us: 0,
        pressure: 0.0,
        flags: 2,
    }];
    assert_eq!(unsafe { sm_append(h.0, forged.as_ptr(), 1) }, 1);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.active_points, 0);
}
