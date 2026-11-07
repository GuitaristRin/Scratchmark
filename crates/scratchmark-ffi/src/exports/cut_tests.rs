use super::*;
use crate::{sm_begin, sm_destroy, sm_features, sm_finish, sm_undo};
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

fn s(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn line(h: u64, y: f64) {
    assert_eq!(sm_begin(h, 2.0, 0x11223344), 0);
    let points = [s(0.0, y, 0), s(10.0, y, 10)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

fn two_strokes() -> Handle {
    let h = Handle::new();
    line(h.0, 0.0);
    line(h.0, 100.0);
    h
}

#[test]
fn features_report_atomic_cut() {
    assert_eq!(sm_features() & 16384, 16384);
}

#[test]
fn cut_writes_clipboard_and_removes_in_one_undo_entry() {
    let h = two_strokes();
    let before = state(h.0);

    // 查询模式：只回报字节数，不剪切。
    let mut required = 0;
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                [0u32].as_ptr(),
                1,
                ptr::null_mut(),
                0,
                &mut required,
            )
        },
        4
    );
    assert!(required > 0);
    assert_eq!(state(h.0).stroke_count, 2, "查询模式不能剪切");
    assert_eq!(state(h.0).revision, before.revision);

    // 容量不足：同样不剪切。
    let mut small = vec![0u8; 4];
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                [0u32].as_ptr(),
                1,
                small.as_mut_ptr(),
                4,
                &mut written,
            )
        },
        4
    );
    assert_eq!(written, required);
    assert_eq!(state(h.0).stroke_count, 2);

    // 正式剪切。
    let mut bytes = vec![0u8; required as usize];
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                [0u32].as_ptr(),
                1,
                bytes.as_mut_ptr(),
                required,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written, required);
    let after = state(h.0);
    assert_eq!(after.stroke_count, 1);
    assert_eq!(after.revision, before.revision + 1);

    // 一次撤销整体恢复。
    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 2);

    // 剪贴板内容能粘贴回原几何。
    let target = Handle::new();
    let revision = state(target.0).revision;
    assert_eq!(
        unsafe {
            sm_paste(
                target.0,
                revision,
                bytes.as_ptr(),
                bytes.len() as u32,
                ptr::null(),
            )
        },
        0
    );
    let mut info = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(target.0, state(target.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_y, -1.0);
}

#[test]
fn cut_multiple_and_out_of_order_indices() {
    let h = Handle::new();
    for index in 0..4 {
        line(h.0, index as f64 * 10.0);
    }
    let before = state(h.0);
    let mut required = 0;
    let indices = [3u32, 1];
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                indices.as_ptr(),
                2,
                ptr::null_mut(),
                0,
                &mut required,
            )
        },
        4
    );
    let mut bytes = vec![0u8; required as usize];
    let mut written = 0;
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                indices.as_ptr(),
                2,
                bytes.as_mut_ptr(),
                required,
                &mut written,
            )
        },
        0
    );
    assert_eq!(state(h.0).stroke_count, 2);
    // 剩下的是原来的第 0、2 笔。
    let mut info = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_y, -1.0);
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 1, &mut info) },
        0
    );
    assert_eq!(info.min_y, 19.0);

    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 4);
}

#[test]
fn cut_rejects_bad_input_without_touching_the_document() {
    let h = two_strokes();
    let before = state(h.0);
    let mut bytes = vec![0u8; 512];
    let mut written = 0;

    // 空列表、越界、重复。
    for (indices, count) in [(vec![0u32], 0u32), (vec![9u32], 1), (vec![0u32, 0], 2)] {
        assert_eq!(
            unsafe {
                sm_cut_strokes(
                    h.0,
                    before.revision,
                    indices.as_ptr(),
                    count,
                    bytes.as_mut_ptr(),
                    512,
                    &mut written,
                )
            },
            1
        );
    }
    // 过期 revision 与空指针。
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision + 1,
                [0u32].as_ptr(),
                1,
                bytes.as_mut_ptr(),
                512,
                &mut written,
            )
        },
        5
    );
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                before.revision,
                [0u32].as_ptr(),
                1,
                bytes.as_mut_ptr(),
                512,
                ptr::null_mut(),
            )
        },
        1
    );
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                0,
                before.revision,
                [0u32].as_ptr(),
                1,
                bytes.as_mut_ptr(),
                512,
                &mut written,
            )
        },
        2
    );
    assert_eq!(state(h.0), before);

    // 活动笔划期间拒绝。
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let points = [s(0.0, 0.0, 0)];
    assert_eq!(unsafe { sm_append(h.0, points.as_ptr(), 1) }, 0);
    let active = state(h.0).revision;
    assert_eq!(
        unsafe {
            sm_cut_strokes(
                h.0,
                active,
                [0u32].as_ptr(),
                1,
                bytes.as_mut_ptr(),
                512,
                &mut written,
            )
        },
        3
    );
    assert_eq!(state(h.0).stroke_count, 2);
}
