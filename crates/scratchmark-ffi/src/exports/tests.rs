use super::*;
use crate::{sm_abi_version, sm_begin, sm_cancel, sm_destroy, sm_finish};
use std::{ptr, thread};

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

#[test]
fn bad_pointers_counts_and_handles_return_errors() {
    assert_eq!(sm_abi_version(), 1);
    assert_eq!(
        unsafe { sm_create(ptr::null_mut()) },
        Error::InvalidArgument as u32
    );
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 1.0, 0), 0);
    assert_eq!(unsafe { sm_append(h.0, ptr::null(), 1) }, 1);
    assert_eq!(unsafe { sm_append(h.0, ptr::null(), u32::MAX) }, 8);
    assert_eq!(unsafe { sm_append(h.0, ptr::null(), 0) }, 0);
    let storage = [0u64; 8];
    let misaligned = unsafe { storage.as_ptr().cast::<u8>().add(1).cast::<Sample>() };
    assert_eq!(unsafe { sm_append(h.0, misaligned, 1) }, 1);
    assert_eq!(sm_cancel(h.0), 0);
    assert_eq!(sm_destroy(h.0), 0);
    assert_eq!(sm_destroy(h.0), 2);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 2);
    let next = Handle::new();
    assert_ne!(h.0, next.0);
}

#[test]
fn insufficient_capacity_and_stale_read_preserve_output() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    assert_eq!(unsafe { sm_append(h.0, &sample(), 1) }, 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(sm_finish(h.0), 0);
    let mut output = [sample()];
    let mut n = 99;
    assert_eq!(
        unsafe { sm_read_points(h.0, state.revision, 0, 0, output.as_mut_ptr(), 1, &mut n) },
        5
    );
    assert_eq!((n, output[0]), (99, sample()));
    let mut required = 0;
    assert_eq!(
        unsafe { sm_save(h.0, ptr::null_mut(), 0, &mut required) },
        4
    );
    assert_eq!(required, 64);
    let mut small = [0xaau8; 63];
    assert_eq!(
        unsafe { sm_save(h.0, small.as_mut_ptr(), 63, &mut required) },
        4
    );
    assert_eq!(small, [0xaa; 63]);
    let mut bytes = vec![0; required as usize];
    assert_eq!(
        unsafe { sm_save(h.0, bytes.as_mut_ptr(), required, &mut required) },
        0
    );
    let second = Handle::new();
    assert_eq!(unsafe { sm_load(second.0, bytes.as_ptr(), required) }, 0);
    assert_eq!(unsafe { sm_get_state(second.0, &mut state) }, 0);
    assert_eq!(state.stroke_count, 1);
}

#[test]
fn simultaneous_calls_on_one_handle_do_not_lose_samples() {
    let h = Handle::new();
    assert_eq!(sm_begin(h.0, 1.0, 0), 0);
    thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    for _ in 0..100 {
                        assert_eq!(unsafe { sm_append(h.0, &sample(), 1) }, 0);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
    });
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.active_points, 400);
}

#[test]
fn unwind_is_contained_and_poisoned_engine_can_be_destroyed() {
    let h = Handle::new();
    assert_eq!(
        crate::guard(|| crate::with_doc(h.0, |_| panic!("ABI containment test"))),
        9
    );
    assert_eq!(sm_begin(h.0, 1.0, 0), 9);
    assert_eq!(sm_destroy(h.0), 0);
}
