use super::*;
use crate::{sm_begin, sm_destroy, sm_features, sm_finish};
use std::ptr;

#[test]
fn features_report_host_alloc() {
    assert_eq!(sm_features() & 65536, 65536);
}

#[test]
fn alloc_returns_usable_aligned_memory() {
    let pointer = sm_alloc(64);
    assert!(!pointer.is_null());
    // 对齐 8 字节：宿主会往里写 f64 / u64。
    assert_eq!(pointer as usize % 8, 0);
    unsafe {
        std::ptr::write_bytes(pointer, 0xAB, 64);
        assert_eq!(*pointer, 0xAB);
        assert_eq!(*pointer.add(63), 0xAB);
        sm_free(pointer, 64);
    }
}

#[test]
fn alloc_rejects_zero_and_oversized_requests() {
    assert!(sm_alloc(0).is_null());
    assert!(sm_alloc(64 * 1024 * 1024 + 1).is_null());
    assert!(sm_alloc(u32::MAX).is_null());
}

#[test]
fn free_tolerates_null_and_bad_sizes() {
    unsafe {
        sm_free(ptr::null_mut(), 16);
        sm_free(ptr::null_mut(), 0);
        // 超上限的 size 直接忽略，不尝试 dealloc。
        let pointer = sm_alloc(16);
        assert!(!pointer.is_null());
        sm_free(pointer, 0);
        sm_free(pointer, 16);
    }
}

#[test]
fn alloc_can_hold_a_snapshot_output_buffer() {
    // 实际用途：把 sm_save 的输出写进宿主申请的内存。
    let mut handle = 0;
    assert_eq!(unsafe { sm_create(&mut handle) }, 0);
    assert_eq!(sm_begin(handle, 2.0, 0x11223344), 0);
    let points = [
        Sample {
            x: 0.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        },
        Sample {
            x: 10.0,
            y: 0.0,
            time_us: 10,
            pressure: 0.0,
            flags: 0,
        },
    ];
    assert_eq!(unsafe { sm_append(handle, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(handle), 0);

    let mut required = 0;
    assert_eq!(
        unsafe { sm_save(handle, ptr::null_mut(), 0, &mut required) },
        4
    );
    let buffer = sm_alloc(required);
    assert!(!buffer.is_null());
    assert_eq!(
        unsafe { sm_save(handle, buffer, required, &mut required) },
        0
    );
    let bytes = unsafe { std::slice::from_raw_parts(buffer, required as usize) };
    assert_eq!(&bytes[..8], b"SCRATCH\0");
    unsafe { sm_free(buffer, required) };
    sm_destroy(handle);
}
