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

fn axes(tilt: f32, orientation: f32) -> SampleAxes {
    SampleAxes { tilt, orientation }
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

#[test]
fn features_report_stylus_axes() {
    assert_eq!(sm_features() & 512, 512);
}

#[test]
fn append_axes_round_trips_through_the_c_abi() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0x11223344), 0);
    let samples = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 1000)];
    let values = [axes(0.0, 0.0), axes(0.75, -2.0)];
    assert_eq!(
        unsafe { sm_append_axes(h.0, samples.as_ptr(), values.as_ptr(), 2) },
        0
    );
    let revision = state(h.0).revision;

    let mut out = [SampleAxes::default(); 2];
    let mut written = 99;
    assert_eq!(
        unsafe { sm_read_axes(h.0, revision, 0, 0, out.as_mut_ptr(), 2, &mut written) },
        0
    );
    assert_eq!(written, 2);
    assert_eq!(out, values);

    // 分段读取与容量语义。
    let mut one = [SampleAxes::default(); 1];
    assert_eq!(
        unsafe { sm_read_axes(h.0, revision, 0, 1, one.as_mut_ptr(), 1, &mut written) },
        0
    );
    assert_eq!(written, 1);
    assert_eq!(one[0], values[1]);
    assert_eq!(
        unsafe { sm_read_axes(h.0, revision, 0, 2, one.as_mut_ptr(), 1, &mut written) },
        0
    );
    assert_eq!(written, 0);
    assert_eq!(
        unsafe { sm_read_axes(h.0, revision, 0, 3, one.as_mut_ptr(), 1, &mut written) },
        1
    );

    // 点与轴向一一对应。
    let mut points = [sample(0.0, 0.0, 0); 2];
    assert_eq!(
        unsafe { sm_read_points(h.0, revision, 0, 0, points.as_mut_ptr(), 2, &mut written) },
        0
    );
    assert_eq!(points[1].x, 10.0);
    assert_eq!(sm_finish(h.0), 0);
}

#[test]
fn invalid_axes_are_rejected_atomically() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let samples = [sample(0.0, 0.0, 0), sample(1.0, 0.0, 1)];
    for bad in [
        axes(2.0, 0.0),
        axes(-0.1, 0.0),
        axes(f32::NAN, 0.0),
        axes(0.0, 4.0),
    ] {
        assert_eq!(
            unsafe { sm_append_axes(h.0, samples.as_ptr(), [bad, bad].as_ptr(), 2) },
            1
        );
    }
    // 空指针与过期 revision。
    assert_eq!(
        unsafe { sm_append_axes(h.0, samples.as_ptr(), ptr::null(), 2) },
        1
    );
    assert_eq!(state(h.0).active_points, 0);
    assert_eq!(
        unsafe {
            sm_read_axes(
                h.0,
                state(h.0).revision + 1,
                0,
                0,
                ptr::null_mut(),
                0,
                &mut 0,
            )
        },
        5
    );
}

#[test]
fn snapshot_with_axes_uses_format_three_and_reloads() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let samples = [sample(0.0, 0.0, 0), sample(5.0, 5.0, 100)];
    let values = [axes(0.4, 1.25), axes(0.9, -0.5)];
    assert_eq!(
        unsafe { sm_append_axes(h.0, samples.as_ptr(), values.as_ptr(), 2) },
        0
    );
    assert_eq!(sm_finish(h.0), 0);

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
    assert_eq!(&bytes[..8], b"SCRATCH3");

    let other = Handle::new();
    assert_eq!(
        unsafe { sm_load(other.0, bytes.as_ptr(), bytes.len() as u32) },
        0
    );
    let revision = state(other.0).revision;
    let mut out = [SampleAxes::default(); 2];
    let mut written = 0;
    assert_eq!(
        unsafe { sm_read_axes(other.0, revision, 0, 0, out.as_mut_ptr(), 2, &mut written) },
        0
    );
    assert_eq!(out, values);
}
