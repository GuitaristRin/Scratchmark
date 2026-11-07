use super::*;
use crate::{sm_begin, sm_destroy, sm_features};
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

fn flat_brush() -> BrushOptions {
    BrushOptions {
        alpha: 1.0,
        min_width_ratio: 1.0,
        pressure_enabled: 0,
        reserved: 0,
    }
}

fn options(horizon_us: f64, refresh_rate_hz: f64) -> PredictionOptionsEx {
    PredictionOptionsEx {
        horizon_us,
        max_distance: 10_000.0,
        count: 4,
        reserved: 0,
        brush: flat_brush(),
        refresh_rate_hz,
    }
}

fn active_line(h: u64) -> u64 {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [
        sample(0.0, 0.0, 0),
        sample(50.0, 0.0, 500),
        sample(100.0, 0.0, 1000),
    ];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 3) }, 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state.revision
}

#[test]
fn features_report_prediction_refresh() {
    assert_eq!(sm_features() & 8192, 8192);
}

#[test]
fn zero_refresh_rate_matches_the_m10_export() {
    let h = Handle::new();
    let revision = active_line(h.0);

    let mut legacy = [BrushPoint::default(); 4];
    let mut legacy_written = 0;
    let base = PredictionOptions {
        horizon_us: 1000.0,
        max_distance: 10_000.0,
        count: 4,
        reserved: 0,
        brush: flat_brush(),
    };
    assert_eq!(
        unsafe {
            sm_predict(
                h.0,
                revision,
                &base,
                legacy.as_mut_ptr(),
                4,
                &mut legacy_written,
            )
        },
        0
    );

    let mut extended = [BrushPoint::default(); 4];
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                &options(1000.0, 0.0),
                extended.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written, legacy_written);
    assert_eq!(extended, legacy);
}

#[test]
fn refresh_rate_shortens_the_prediction() {
    let h = Handle::new();
    let revision = active_line(h.0);

    let mut long = [BrushPoint::default(); 4];
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                &options(16_000.0, 0.0),
                long.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        0
    );
    let mut capped = [BrushPoint::default(); 4];
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                &options(16_000.0, 240.0),
                capped.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        0
    );
    assert!(capped[3].x < long[3].x);
    // 速度 0.1/µs：16 ms → +1600；240 Hz 一帧 ≈ 4166.67 µs → +416.67。
    assert!((long[3].x - 1700.0).abs() < 1e-6);
    assert!((capped[3].x - 516.666_666_7).abs() < 1e-6);

    // 容量不足：回报数量，不写数组。
    let mut small = [BrushPoint::default(); 1];
    small[0].x = 77.0;
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                &options(16_000.0, 240.0),
                small.as_mut_ptr(),
                1,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 4);
    assert_eq!(small[0].x, 77.0);
}

#[test]
fn invalid_refresh_rates_are_rejected_without_output() {
    let h = Handle::new();
    let revision = active_line(h.0);
    let mut points = [BrushPoint::default(); 4];
    let mut written = 99;

    for rate in [-1.0, f64::NAN, f64::INFINITY, 10_001.0] {
        assert_eq!(
            unsafe {
                sm_predict_ex(
                    h.0,
                    revision,
                    &options(16_000.0, rate),
                    points.as_mut_ptr(),
                    4,
                    &mut written,
                )
            },
            1
        );
    }
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                ptr::null(),
                points.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision + 1,
                &options(16_000.0, 240.0),
                points.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        5
    );
    assert_eq!(
        unsafe {
            sm_predict_ex(
                h.0,
                revision,
                &options(16_000.0, 240.0),
                points.as_mut_ptr(),
                4,
                ptr::null_mut(),
            )
        },
        1
    );
    assert_eq!(written, 99);
}
