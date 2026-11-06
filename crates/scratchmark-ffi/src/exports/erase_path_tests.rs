use super::*;
use crate::{sm_begin, sm_cancel, sm_destroy, sm_features, sm_finish, sm_redo, sm_undo};
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

fn sample(x: f64, y: f64) -> Sample {
    Sample {
        x,
        y,
        time_us: 0,
        pressure: 0.0,
        flags: 0,
    }
}

fn commit(h: u64, from: (f64, f64), to: (f64, f64)) {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [sample(from.0, from.1), sample(to.0, to.1)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn features_report_partial_erase() {
    assert_eq!(sm_features() & 8, 8);
}

#[test]
fn erase_path_splits_and_reports_through_the_c_abi() {
    let h = Handle::new();
    commit(h.0, (0.0, 0.0), (10.0, 0.0));
    let before = state(h.0);

    let path = [point(5.0, 0.0)];
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 1, 2.0) },
        0
    );

    let after = state(h.0);
    assert_eq!(after.stroke_count, 2);
    assert_eq!(after.can_undo, 1);
    assert_ne!(after.revision, before.revision);

    let mut info = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(h.0, after.revision, 0, &mut info) },
        0
    );
    assert_eq!(info.width, 2.0);
    assert_eq!(info.color, 0x11223344);
    assert_eq!(info.point_count, 2);
    assert!((info.max_x - 4.0).abs() < 1e-9);

    assert_eq!(
        unsafe { sm_get_stroke(h.0, after.revision, 1, &mut info) },
        0
    );
    assert!((info.min_x - 6.0).abs() < 1e-9);

    assert_eq!(sm_undo(h.0), 0);
    let undone = state(h.0);
    assert_eq!(undone.stroke_count, 1);
    assert_eq!(
        unsafe { sm_get_stroke(h.0, undone.revision, 0, &mut info) },
        0
    );
    assert_eq!(info.point_count, 2);
    assert!((info.min_x + 1.0).abs() < 1e-9);

    assert_eq!(sm_redo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 2);
}

#[test]
fn erase_path_rejects_bad_arguments_without_changing_state() {
    let h = Handle::new();
    commit(h.0, (0.0, 0.0), (10.0, 0.0));
    let before = state(h.0);
    let path = [point(5.0, 0.0)];

    // 指针与计数先于句柄语义：NULL 与 0 计数、超上限分别报参数与限额错误。
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, ptr::null(), 1, 1.0) },
        1
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 0, 1.0) },
        1
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 65_537, 1.0) },
        8
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision - 1, path.as_ptr(), 1, 1.0) },
        5
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 1, 0.0) },
        1
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 1, f64::NAN) },
        1
    );
    let nan_path = [point(f64::NAN, 0.0)];
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, nan_path.as_ptr(), 1, 1.0) },
        1
    );
    assert_eq!(
        unsafe { sm_erase_path(h.0, before.revision, path.as_ptr(), 1, 1e9 + 1.0) },
        1
    );
    assert_eq!(state(h.0).revision, before.revision);
    assert_eq!(state(h.0).stroke_count, before.stroke_count);

    // 活动笔划期间不可擦除。
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let active = state(h.0);
    assert_eq!(
        unsafe { sm_erase_path(h.0, active.revision, path.as_ptr(), 1, 1.0) },
        3
    );
    assert_eq!(sm_cancel(h.0), 0);

    // 无效果的成功调用不递增 revision。
    let idle = state(h.0);
    let far = [point(1000.0, 1000.0)];
    assert_eq!(
        unsafe { sm_erase_path(h.0, idle.revision, far.as_ptr(), 1, 1.0) },
        0
    );
    assert_eq!(state(h.0).revision, idle.revision);
    assert_eq!(
        unsafe { sm_erase_path(0, idle.revision, path.as_ptr(), 1, 1.0) },
        2
    );
}
