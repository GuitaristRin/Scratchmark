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

fn line(h: u64, from: (f64, f64), to: (f64, f64)) {
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

fn square() -> [Point; 4] {
    [
        Point { x: 100.0, y: 100.0 },
        Point { x: 300.0, y: 100.0 },
        Point { x: 300.0, y: 300.0 },
        Point { x: 100.0, y: 300.0 },
    ]
}

#[test]
fn features_report_selection() {
    assert_eq!(sm_features() & 16, 16);
}

#[test]
fn lasso_query_mode_reports_required_count_and_writes_nothing() {
    let h = Handle::new();
    line(h.0, (150.0, 150.0), (200.0, 150.0));
    line(h.0, (50.0, 200.0), (150.0, 200.0));
    line(h.0, (500.0, 500.0), (600.0, 500.0));
    let revision = state(h.0).revision;
    let path = square();

    let mut written = 99;
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                4,
                ptr::null_mut(),
                0,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 2);

    let mut indices = [99u32; 8];
    written = 99;
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                4,
                indices.as_mut_ptr(),
                8,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written, 2);
    assert_eq!(&indices[..2], &[0, 1]);
    assert_eq!(indices[2], 99);

    // 容量不足：写出所需数量，数组不动。
    let mut small = [77u32; 1];
    written = 99;
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                4,
                small.as_mut_ptr(),
                1,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, 2);
    assert_eq!(small[0], 77);

    // 命中为零时是成功，不是容量不足。
    let far = [
        Point { x: 900.0, y: 900.0 },
        Point { x: 950.0, y: 900.0 },
        Point { x: 950.0, y: 950.0 },
    ];
    written = 99;
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                far.as_ptr(),
                3,
                small.as_mut_ptr(),
                1,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written, 0);
}

#[test]
fn lasso_rejects_bad_arguments_without_output() {
    let h = Handle::new();
    line(h.0, (150.0, 150.0), (200.0, 150.0));
    let revision = state(h.0).revision;
    let path = square();
    let mut indices = [77u32; 4];
    let mut written = 99;

    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                ptr::null(),
                4,
                indices.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                4,
                indices.as_mut_ptr(),
                4,
                ptr::null_mut(),
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision + 1,
                path.as_ptr(),
                4,
                indices.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        5
    );
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                2,
                indices.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                65_537,
                indices.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        8
    );
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                h.0,
                revision,
                path.as_ptr(),
                4,
                ptr::null_mut(),
                4,
                &mut written,
            )
        },
        1
    );
    assert_eq!(written, 99);
    assert_eq!(indices, [77; 4]);

    // 句柄无效优先于参数之外的语义检查。
    assert_eq!(
        unsafe {
            sm_lasso_hits(
                0,
                revision,
                path.as_ptr(),
                4,
                indices.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        2
    );
}

#[test]
fn erase_indices_deletes_atomically_and_one_undo_restores() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    line(h.0, (100.0, 0.0), (110.0, 0.0));
    line(h.0, (200.0, 0.0), (210.0, 0.0));
    let revision = state(h.0).revision;
    let indices = [2u32, 0u32];

    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision, indices.as_ptr(), 2) },
        0
    );
    assert_eq!(state(h.0).stroke_count, 1);
    let mut info = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_x, 99.0);

    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 3);
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_x, -1.0);

    assert_eq!(sm_redo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 1);
}

#[test]
fn erase_indices_and_clear_validate_state_revision_and_payload() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    line(h.0, (100.0, 0.0), (110.0, 0.0));
    let revision = state(h.0).revision;

    let duplicated = [0u32, 0u32];
    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision, duplicated.as_ptr(), 2) },
        1
    );
    let out_of_range = [0u32, 5u32];
    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision, out_of_range.as_ptr(), 2) },
        1
    );
    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision + 1, duplicated.as_ptr(), 1) },
        5
    );
    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision, ptr::null(), 1) },
        1
    );
    assert_eq!(state(h.0).stroke_count, 2);

    // 空列表是空操作：不改 revision。
    assert_eq!(
        unsafe { sm_erase_indices(h.0, revision, ptr::null(), 0) },
        0
    );
    assert_eq!(state(h.0).revision, revision);

    // 活动笔划期间拒绝。
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    assert_eq!(
        unsafe { sm_erase_indices(h.0, state(h.0).revision, duplicated.as_ptr(), 1) },
        3
    );
    assert_eq!(unsafe { sm_clear(h.0, state(h.0).revision) }, 3);
    assert_eq!(sm_cancel(h.0), 0);

    assert_eq!(unsafe { sm_clear(h.0, revision + 1) }, 5);
    assert_eq!(unsafe { sm_clear(h.0, state(h.0).revision) }, 0);
    assert_eq!(state(h.0).stroke_count, 0);
    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 2);

    // 空文档清空：成功且不递增 revision。
    assert_eq!(unsafe { sm_clear(h.0, state(h.0).revision) }, 0);
    let empty = state(h.0);
    assert_eq!(empty.stroke_count, 0);
    assert_eq!(unsafe { sm_clear(h.0, empty.revision) }, 0);
    assert_eq!(state(h.0).revision, empty.revision);
    assert_eq!(unsafe { sm_clear(0, empty.revision) }, 2);
}
