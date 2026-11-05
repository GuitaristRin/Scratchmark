use super::*;
use crate::{sm_begin, sm_cancel, sm_destroy, sm_features, sm_finish};
use scratchmark_core::{Bounds, Point, Viewport};
use std::ptr;

#[test]
fn width_underflow_keeps_ffi_output_unchanged() {
    // 参 spec/M3.md §页面坐标与视口坐标：错误不得覆盖调用方输出。
    let tiny = Viewport {
        scale: f64::from_bits(1),
        ..Viewport::default()
    };
    let large = Viewport {
        scale: 2.0,
        ..Viewport::default()
    };
    let mut output = 17.0;
    assert_eq!(
        unsafe { sm_width_page_to_viewport(&tiny, 0.5, &mut output) },
        1
    );
    assert_eq!(output, 17.0);
    assert_eq!(
        unsafe { sm_width_viewport_to_page(&large, f64::from_bits(1), &mut output) },
        1
    );
    assert_eq!(output, 17.0);
}

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

fn sample() -> Sample {
    Sample {
        x: 1.0,
        y: 2.0,
        time_us: 0,
        pressure: 0.0,
        flags: 0,
    }
}

fn viewport() -> Viewport {
    Viewport {
        scale: 2.0,
        offset_x: 10.0,
        offset_y: 20.0,
        reserved: 0,
    }
}

#[test]
fn features_report_page_and_viewport_capability() {
    assert_eq!(sm_features() & 0b10, 0b10);
    assert_eq!(sm_features() & 1, 1);
}

#[test]
fn page_id_reads_writes_and_revisions_follow_the_spec() {
    let h = Handle::new();
    let mut page = 7;
    assert_eq!(unsafe { sm_get_page_id(h.0, &mut page) }, 0);
    assert_eq!(page, 0);
    assert_eq!(unsafe { sm_get_page_id(h.0, ptr::null_mut()) }, 1);
    assert_eq!(unsafe { sm_get_page_id(0xdead, &mut page) }, 2);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    let before = state.revision;
    assert_eq!(unsafe { sm_set_page_id(h.0, 42) }, 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.revision, before + 1);
    assert_eq!(unsafe { sm_set_page_id(h.0, 42) }, 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.revision, before + 1);
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    assert_eq!(unsafe { sm_set_page_id(h.0, 43) }, 3);
    assert_eq!(sm_cancel(h.0), 0);
    assert_eq!(unsafe { sm_set_page_id(h.0, 0) }, 0);
    assert_eq!(unsafe { sm_get_page_id(h.0, &mut page) }, 0);
    assert_eq!(page, 0);
    assert_eq!(unsafe { sm_set_page_id(0xdead, 1) }, 2);
}

#[test]
fn snapshot_with_page_identity_roundtrips_through_ffi() {
    let h = Handle::new();
    assert_eq!(unsafe { sm_set_page_id(h.0, 42) }, 0);
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    assert_eq!(unsafe { sm_append(h.0, &sample(), 1) }, 0);
    assert_eq!(sm_finish(h.0), 0);
    let mut required = 0;
    assert_eq!(
        unsafe { sm_save(h.0, ptr::null_mut(), 0, &mut required) },
        4
    );
    // 格式 2 头比格式 1 多 8 字节页面标识。
    assert_eq!(required, 72);
    let mut bytes = vec![0; required as usize];
    assert_eq!(
        unsafe { sm_save(h.0, bytes.as_mut_ptr(), required, &mut required) },
        0
    );
    assert_eq!(&bytes[..8], b"SCRATCH2");
    let other = Handle::new();
    assert_eq!(unsafe { sm_set_page_id(other.0, 43) }, 0);
    assert_eq!(unsafe { sm_load(other.0, bytes.as_ptr(), required) }, 6);
    let fresh = Handle::new();
    assert_eq!(unsafe { sm_load(fresh.0, bytes.as_ptr(), required) }, 0);
    let mut page = 0;
    assert_eq!(unsafe { sm_get_page_id(fresh.0, &mut page) }, 0);
    assert_eq!(page, 42);
}

#[test]
fn viewport_transforms_validate_pointers_values_and_outputs() {
    let v = viewport();
    let p = Point { x: 3.0, y: 4.0 };
    let mut out = Point { x: 99.0, y: 98.0 };
    assert_eq!(unsafe { sm_page_to_viewport(&v, &p, &mut out) }, 0);
    assert_eq!((out.x, out.y), (16.0, 28.0));
    assert_eq!(unsafe { sm_viewport_to_page(&v, &out, &mut out) }, 0);
    assert_eq!((out.x, out.y), (3.0, 4.0));
    // NULL 与不对齐指针一律 INVALID_ARGUMENT。
    assert_eq!(unsafe { sm_page_to_viewport(ptr::null(), &p, &mut out) }, 1);
    assert_eq!(unsafe { sm_page_to_viewport(&v, ptr::null(), &mut out) }, 1);
    assert_eq!(unsafe { sm_page_to_viewport(&v, &p, ptr::null_mut()) }, 1);
    let storage = [0u64; 8];
    let misaligned = unsafe { storage.as_ptr().cast::<u8>().add(1).cast::<Viewport>() };
    assert_eq!(unsafe { sm_page_to_viewport(misaligned, &p, &mut out) }, 1);
    let mut width = 0.0;
    assert_eq!(unsafe { sm_width_page_to_viewport(&v, 5.0, &mut width) }, 0);
    assert_eq!(width, 10.0);
    assert_eq!(
        unsafe { sm_width_viewport_to_page(&v, 10.0, &mut width) },
        0
    );
    assert_eq!(width, 5.0);
    assert_eq!(unsafe { sm_width_page_to_viewport(&v, 0.0, &mut width) }, 1);
    assert_eq!(width, 5.0);
    let bounds = Bounds {
        min_x: -1.0,
        min_y: -2.0,
        max_x: 3.0,
        max_y: 4.0,
    };
    let mut out_bounds = Bounds::default();
    assert_eq!(
        unsafe { sm_bounds_page_to_viewport(&v, &bounds, &mut out_bounds) },
        0
    );
    assert_eq!(
        (
            out_bounds.min_x,
            out_bounds.min_y,
            out_bounds.max_x,
            out_bounds.max_y
        ),
        (8.0, 16.0, 16.0, 28.0)
    );
    // 失败不写输出：哨兵保持原值。
    let mut sentinel = Point { x: 99.0, y: 98.0 };
    let bad = Point {
        x: f64::NAN,
        y: 0.0,
    };
    assert_eq!(unsafe { sm_page_to_viewport(&v, &bad, &mut sentinel) }, 1);
    assert_eq!((sentinel.x, sentinel.y), (99.0, 98.0));
    let mut invalid = viewport();
    invalid.scale = 0.0;
    assert_eq!(
        unsafe { sm_page_to_viewport(&invalid, &p, &mut sentinel) },
        1
    );
    invalid = viewport();
    invalid.reserved = 3;
    assert_eq!(
        unsafe { sm_viewport_to_page(&invalid, &p, &mut sentinel) },
        1
    );
    assert_eq!((sentinel.x, sentinel.y), (99.0, 98.0));
}
