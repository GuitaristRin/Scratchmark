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

fn options(count: u32, horizon_us: f64, max_distance: f64) -> PredictionOptions {
    PredictionOptions {
        horizon_us,
        max_distance,
        count,
        reserved: 0,
        brush: BrushOptions {
            alpha: 1.0,
            min_width_ratio: 1.0,
            pressure_enabled: 0,
            reserved: 0,
        },
    }
}

fn active_stroke(h: u64) {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [
        sample(0.0, 0.0, 0),
        sample(50.0, 0.0, 500),
        sample(100.0, 0.0, 1000),
    ];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 3) }, 0);
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

#[test]
fn features_report_prediction() {
    assert_eq!(sm_features() & 256, 256);
}

#[test]
fn predict_returns_ordered_points_and_query_mode() {
    let h = Handle::new();
    active_stroke(h.0);
    let revision = state(h.0).revision;
    let opts = options(4, 1000.0, 1000.0);

    // 查询模式：容量 0 只回报数量。
    let mut written = 99;
    assert_eq!(
        unsafe { sm_predict(h.0, revision, &opts, ptr::null_mut(), 0, &mut written) },
        4
    );
    assert_eq!(written, 4);

    let mut points = [BrushPoint::default(); 8];
    written = 99;
    assert_eq!(
        unsafe { sm_predict(h.0, revision, &opts, points.as_mut_ptr(), 8, &mut written) },
        0
    );
    assert_eq!(written, 4);
    // 速度 0.1 单位/微秒 × 1000 微秒，均分 4 段。
    assert!((points[0].x - 125.0).abs() < 1e-9);
    assert!((points[3].x - 200.0).abs() < 1e-9);
    assert!((points[3].y).abs() < 1e-9);
    assert!((points[0].width - 2.0).abs() < 1e-9);

    // 容量不足：写出所需数量，数组不动。
    let mut small = [BrushPoint::default(); 1];
    small[0].x = 77.0;
    written = 99;
    assert_eq!(
        unsafe { sm_predict(h.0, revision, &opts, small.as_mut_ptr(), 1, &mut written) },
        4
    );
    assert_eq!(written, 4);
    assert_eq!(small[0].x, 77.0);

    // 预测不改变 revision 与活动点。
    let after = state(h.0);
    assert_eq!(after.revision, revision);
    assert_eq!(after.active_points, 3);
}

#[test]
fn predict_without_active_stroke_is_ok_and_empty() {
    let h = Handle::new();
    let revision = state(h.0).revision;
    let opts = options(4, 1000.0, 100.0);
    let mut points = [BrushPoint::default(); 4];
    let mut written = 99;
    assert_eq!(
        unsafe { sm_predict(h.0, revision, &opts, points.as_mut_ptr(), 4, &mut written) },
        0
    );
    assert_eq!(written, 0);

    // 提交后同样为空。
    active_stroke(h.0);
    assert_eq!(sm_finish(h.0), 0);
    let revision = state(h.0).revision;
    assert_eq!(
        unsafe { sm_predict(h.0, revision, &opts, points.as_mut_ptr(), 4, &mut written) },
        0
    );
    assert_eq!(written, 0);
}

#[test]
fn predict_rejects_bad_options_pointers_and_revisions() {
    let h = Handle::new();
    active_stroke(h.0);
    let revision = state(h.0).revision;
    let good = options(4, 1000.0, 100.0);
    let mut points = [BrushPoint::default(); 4];
    let mut written = 99;

    for bad in [
        options(0, 1000.0, 100.0),
        options(65, 1000.0, 100.0),
        options(4, 0.0, 100.0),
        options(4, 1000.0, 0.0),
        PredictionOptions {
            reserved: 1,
            ..options(4, 1000.0, 100.0)
        },
    ] {
        assert_eq!(
            unsafe { sm_predict(h.0, revision, &bad, points.as_mut_ptr(), 4, &mut written) },
            1
        );
    }
    assert_eq!(
        unsafe {
            sm_predict(
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
            sm_predict(
                h.0,
                revision + 1,
                &good,
                points.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        5
    );
    assert_eq!(
        unsafe {
            sm_predict(
                h.0,
                revision,
                &good,
                points.as_mut_ptr(),
                4,
                ptr::null_mut(),
            )
        },
        1
    );
    assert_eq!(
        unsafe { sm_predict(0, revision, &good, points.as_mut_ptr(), 4, &mut written) },
        2
    );
    assert_eq!(written, 99);
}
