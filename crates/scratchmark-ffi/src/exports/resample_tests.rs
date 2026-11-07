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

fn options(ratio: f64) -> ResampleOptions {
    ResampleOptions {
        spacing_ratio: ratio,
        brush: BrushOptions {
            alpha: 1.0,
            min_width_ratio: 1.0,
            pressure_enabled: 0,
            reserved: 0,
        },
        reserved: 0,
    }
}

/// 一笔直线：线宽 2，从 (0,0) 到 (10,0)。
fn straight(h: u64) -> u64 {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 100)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state.revision
}

#[test]
fn features_report_resample() {
    assert_eq!(sm_features() & 4096, 4096);
}

#[test]
fn resample_reports_counts_and_fills_points() {
    let h = Handle::new();
    let revision = straight(h.0);

    // 查询模式：容量 0 只回报数量。线宽 2、比例 0.5 → 间距 1 → 11 点。
    let mut written = 99;
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                0,
                &options(0.5),
                ptr::null_mut(),
                0,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 11);

    let mut points = vec![BrushPoint::default(); 11];
    written = 99;
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                0,
                &options(0.5),
                points.as_mut_ptr(),
                11,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written, 11);
    assert!((points[0].x - 0.0).abs() < 1e-9);
    assert!((points[5].x - 5.0).abs() < 1e-9);
    assert!((points[10].x - 10.0).abs() < 1e-9);
    assert!((points[10].width - 2.0).abs() < 1e-9);

    // 容量不足：回报所需数量，数组不动。
    let mut small = vec![BrushPoint::default(); 3];
    small[0].x = 77.0;
    written = 99;
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                0,
                &options(0.5),
                small.as_mut_ptr(),
                3,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 11);
    assert_eq!(small[0].x, 77.0);

    // 只读：文档不变。
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.revision, revision);
}

#[test]
fn resample_reads_the_active_stroke_too() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let points = [sample(0.0, 0.0, 0), sample(10.0, 0.0, 100)];
    assert_eq!(unsafe { sm_append(h.0, points.as_ptr(), 2) }, 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);

    // 活动笔划的索引等于 stroke_count。
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                state.revision,
                state.stroke_count,
                &options(0.5),
                ptr::null_mut(),
                0,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 11);
}

#[test]
fn resample_rejects_bad_options_pointers_and_revisions() {
    let h = Handle::new();
    let revision = straight(h.0);
    let good = options(0.5);
    let mut points = vec![BrushPoint::default(); 11];
    let mut written = 99;

    for bad in [
        options(0.0),
        options(-1.0),
        options(f64::NAN),
        ResampleOptions {
            reserved: 1,
            ..options(0.5)
        },
    ] {
        assert_eq!(
            unsafe {
                sm_resample_stroke(
                    h.0,
                    revision,
                    0,
                    &bad,
                    points.as_mut_ptr(),
                    11,
                    &mut written,
                )
            },
            1
        );
    }
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                0,
                ptr::null(),
                points.as_mut_ptr(),
                11,
                &mut written,
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision + 1,
                0,
                &good,
                points.as_mut_ptr(),
                11,
                &mut written,
            )
        },
        5
    );
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                9,
                &good,
                points.as_mut_ptr(),
                11,
                &mut written,
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_resample_stroke(
                h.0,
                revision,
                0,
                &good,
                points.as_mut_ptr(),
                11,
                ptr::null_mut(),
            )
        },
        1
    );
    assert_eq!(
        unsafe { sm_resample_stroke(0, revision, 0, &good, points.as_mut_ptr(), 11, &mut written) },
        2
    );
    assert_eq!(written, 99);
}
