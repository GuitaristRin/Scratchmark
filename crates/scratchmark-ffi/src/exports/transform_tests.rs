use super::*;
use crate::{sm_begin, sm_destroy, sm_features, sm_finish, sm_redo, sm_undo};
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

fn translate(dx: f64, dy: f64) -> Transform {
    Transform {
        dx,
        dy,
        ..Transform::IDENTITY
    }
}

#[test]
fn features_report_transform() {
    assert_eq!(sm_features() & 32, 32);
}

#[test]
fn transform_moves_selection_in_place() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    line(h.0, (100.0, 0.0), (110.0, 0.0));
    let revision = state(h.0).revision;
    let indices = [0u32];
    let shift = translate(10.0, 5.0);

    assert_eq!(
        unsafe { sm_transform_strokes(h.0, revision, indices.as_ptr(), 1, &shift) },
        0
    );
    let after = state(h.0);
    assert_eq!(after.stroke_count, 2);
    let mut info = StrokeInfo::default();
    assert_eq!(
        unsafe { sm_get_stroke(h.0, after.revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_x, 9.0);
    assert_eq!(info.min_y, 4.0);
    // 未被选中的笔划保持原位。
    assert_eq!(
        unsafe { sm_get_stroke(h.0, after.revision, 1, &mut info) },
        0
    );
    assert_eq!(info.min_x, 99.0);

    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_x, -1.0);
    assert_eq!(sm_redo(h.0), 0);
    assert_eq!(
        unsafe { sm_get_stroke(h.0, state(h.0).revision, 0, &mut info) },
        0
    );
    assert_eq!(info.min_x, 9.0);
}

#[test]
fn transform_rejects_null_and_degenerate_matrices() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    let revision = state(h.0).revision;
    let indices = [0u32];
    let flat = Transform {
        m11: 0.0,
        ..Transform::IDENTITY
    };

    assert_eq!(
        unsafe { sm_transform_strokes(h.0, revision, indices.as_ptr(), 1, ptr::null()) },
        1
    );
    assert_eq!(
        unsafe { sm_transform_strokes(h.0, revision, indices.as_ptr(), 1, &flat) },
        1
    );
    assert_eq!(
        unsafe {
            sm_transform_strokes(h.0, revision + 1, indices.as_ptr(), 1, &translate(1.0, 0.0))
        },
        5
    );
    // 空选区是空操作。
    assert_eq!(
        unsafe { sm_transform_strokes(h.0, revision, ptr::null(), 0, &translate(1.0, 0.0)) },
        0
    );
    assert_eq!(state(h.0).revision, revision);
}

#[test]
fn copy_query_then_fetch_and_paste_round_trip() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    line(h.0, (100.0, 0.0), (110.0, 0.0));
    let revision = state(h.0).revision;
    let indices = [0u32, 1u32];

    let mut required = 0u32;
    assert_eq!(
        unsafe {
            sm_copy_strokes(
                h.0,
                revision,
                indices.as_ptr(),
                2,
                ptr::null_mut(),
                0,
                &mut required,
            )
        },
        4
    );
    assert!(required > 8);

    let mut blob = vec![0u8; required as usize];
    assert_eq!(
        unsafe {
            sm_copy_strokes(
                h.0,
                revision,
                indices.as_ptr(),
                2,
                blob.as_mut_ptr(),
                required,
                &mut required,
            )
        },
        0
    );
    assert_eq!(&blob[..8], b"SCRATCH\0");

    // 容量不足时不写数据区，只回报所需大小。
    let mut small = vec![7u8; 4];
    let mut needed = 0u32;
    assert_eq!(
        unsafe {
            sm_copy_strokes(
                h.0,
                revision,
                indices.as_ptr(),
                2,
                small.as_mut_ptr(),
                4,
                &mut needed,
            )
        },
        4
    );
    assert_eq!(needed as usize, blob.len());
    assert_eq!(small, vec![7u8; 4]);

    // 粘贴回同一文档：置顶、一个撤销条目。
    assert_eq!(
        unsafe { sm_paste(h.0, revision, blob.as_ptr(), blob.len() as u32, ptr::null()) },
        0
    );
    let pasted = state(h.0);
    assert_eq!(pasted.stroke_count, 4);
    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(state(h.0).stroke_count, 2);

    // 跨文档粘贴。
    let other = Handle::new();
    assert_eq!(
        unsafe {
            sm_paste(
                other.0,
                state(other.0).revision,
                blob.as_ptr(),
                blob.len() as u32,
                ptr::null(),
            )
        },
        0
    );
    assert_eq!(state(other.0).stroke_count, 2);
}

#[test]
fn paste_rejects_format2_and_bad_payloads() {
    let source = Handle::new();
    let mut page_id = 0u64;
    assert_eq!(unsafe { sm_set_page_id(source.0, 42) }, 0);
    assert_eq!(unsafe { sm_get_page_id(source.0, &mut page_id) }, 0);
    line(source.0, (0.0, 0.0), (10.0, 0.0));
    let mut required = 0u32;
    assert_eq!(
        unsafe { sm_save(source.0, ptr::null_mut(), 0, &mut required) },
        4
    );
    let mut document = vec![0u8; required as usize];
    assert_eq!(
        unsafe { sm_save(source.0, document.as_mut_ptr(), required, &mut required) },
        0
    );
    assert_eq!(&document[..8], b"SCRATCH2");

    let target = Handle::new();
    let revision = state(target.0).revision;
    assert_eq!(
        unsafe {
            sm_paste(
                target.0,
                revision,
                document.as_ptr(),
                document.len() as u32,
                ptr::null(),
            )
        },
        6
    );
    assert_eq!(
        unsafe { sm_paste(target.0, revision, ptr::null(), 4, ptr::null()) },
        1
    );
    assert_eq!(
        unsafe {
            sm_paste(
                target.0,
                revision,
                document.as_ptr(),
                document.len() as u32,
                ptr::null_mut(),
            )
        },
        6
    );
    assert_eq!(state(target.0).revision, revision);
    assert_eq!(
        unsafe { sm_paste(0, revision, document.as_ptr(), 8, ptr::null()) },
        2
    );
}

#[test]
fn transform_and_paste_refuse_active_stroke() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0));
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    let active = state(h.0).revision;
    let indices = [0u32];
    let shift = translate(1.0, 0.0);
    assert_eq!(
        unsafe { sm_transform_strokes(h.0, active, indices.as_ptr(), 1, &shift) },
        3
    );

    // 复制是只读查询，活动笔划期间允许（它只覆盖已提交笔划）。
    let mut required = 0u32;
    assert_eq!(
        unsafe {
            sm_copy_strokes(
                h.0,
                active,
                indices.as_ptr(),
                1,
                ptr::null_mut(),
                0,
                &mut required,
            )
        },
        4
    );
    assert!(required > 8);
}
