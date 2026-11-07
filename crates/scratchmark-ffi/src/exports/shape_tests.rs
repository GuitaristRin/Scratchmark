use super::*;
use crate::{sm_begin, sm_cancel, sm_destroy, sm_features, sm_finish, sm_undo};
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
        pressure: 0.5,
        flags: 1,
    }
}

fn commit(h: u64, points: &[(f64, f64)]) {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let samples: Vec<Sample> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| sample(*x, *y, i as u64 * 100))
        .collect();
    assert_eq!(
        unsafe { sm_append(h, samples.as_ptr(), samples.len() as u32) },
        0
    );
    assert_eq!(sm_finish(h), 0);
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

#[test]
fn features_report_shape() {
    assert_eq!(sm_features() & 64, 64);
}

#[test]
fn regularize_preview_then_apply_through_the_c_abi() {
    let h = Handle::new();
    commit(h.0, &[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]);
    let revision = state(h.0).revision;

    let mut info = ShapeInfo::default();
    // 预览：只测量，不改文档。
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 1, 1, &mut info) },
        0
    );
    assert_eq!(info.kind, 1);
    assert_eq!(info.stroke_count, 1);
    assert_eq!(info.point_count, 2);
    assert!(info.error < 1e-9);
    assert_eq!(state(h.0).revision, revision);
    assert_eq!(state(h.0).stroke_count, 1);

    // 实际替换：一个撤销条目。
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 1, 0, &mut info) },
        0
    );
    let after = state(h.0);
    assert_eq!(after.revision, revision + 1);
    assert_eq!(after.stroke_count, 1);
    assert_eq!(after.can_redo, 0);
    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 1);
    let mut stroke = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut stroke) },
        0
    );
    assert_eq!(stroke.point_count, 3);
    assert_eq!(stroke.width, 2.0);
}

#[test]
fn arrow_regularization_reports_three_strokes() {
    let h = Handle::new();
    commit(h.0, &[(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)]);
    let revision = state(h.0).revision;
    let mut info = ShapeInfo::default();
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 4, 0, &mut info) },
        0
    );
    assert_eq!(info.kind, 4);
    assert_eq!(info.stroke_count, 3);
    assert_eq!(info.point_count, 6);
    assert_eq!(state(h.0).stroke_count, 3);
}

#[test]
fn invalid_kind_flags_and_arguments_are_rejected_without_output() {
    let h = Handle::new();
    commit(h.0, &[(0.0, 0.0), (10.0, 0.0)]);
    let revision = state(h.0).revision;
    let mut info = ShapeInfo {
        kind: 99,
        error: 99.0,
        ..ShapeInfo::default()
    };

    // 未知种类、非法 flags、空输出指针、越界索引、过期 revision。
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 9, 0, &mut info) },
        1
    );
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 1, 2, &mut info) },
        1
    );
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 0, 1, 0, ptr::null_mut()) },
        1
    );
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 7, 1, 1, &mut info) },
        1
    );
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision - 1, 0, 1, 1, &mut info) },
        5
    );
    assert_eq!(info.kind, 99);
    assert_eq!(info.error, 99.0);
    assert_eq!(state(h.0).revision, revision);

    // 退化笔划：错误码是 INVALID_ARGUMENT，输出保持原样。
    commit(h.0, &[(5.0, 5.0)]);
    let revision = state(h.0).revision;
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, revision, 1, 1, 0, &mut info) },
        1
    );
    assert_eq!(info.kind, 99);

    // 活动笔划期间预览允许、实际替换拒绝。
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let active = state(h.0).revision;
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, active, 0, 1, 1, &mut info) },
        0
    );
    assert_eq!(
        unsafe { sm_regularize_stroke(h.0, active, 0, 1, 0, &mut info) },
        3
    );
    assert_eq!(sm_cancel(h.0), 0);
    assert_eq!(
        unsafe { sm_regularize_stroke(0, active, 0, 1, 1, &mut info) },
        2
    );
}
