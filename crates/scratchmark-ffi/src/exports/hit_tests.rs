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

fn sample(x: f64) -> Sample {
    Sample {
        x,
        y: 0.0,
        time_us: 0,
        pressure: 0.0,
        flags: 0,
    }
}

fn commit(h: u64, x: f64) {
    assert_eq!(sm_begin(h, 2.0, 0), 0);
    let points = [sample(x), sample(x + 10.0)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
}

#[test]
fn features_report_hit_erase_and_outputs_are_atomic() {
    assert_eq!(sm_features() & 4, 4);
    let h = Handle::new();
    commit(h.0, 0.0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);

    let mut index = 99;
    let mut found = 99;
    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 5.0, 0.0, 0.0, &mut index, &mut found) },
        0
    );
    assert_eq!((index, found), (0, 1));

    index = 99;
    found = 99;
    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 100.0, 0.0, 0.0, &mut index, &mut found) },
        0
    );
    assert_eq!((index, found), (0, 0));

    index = 99;
    found = 99;
    assert_eq!(
        unsafe {
            sm_hit_test(
                h.0,
                state.revision - 1,
                5.0,
                0.0,
                0.0,
                &mut index,
                &mut found,
            )
        },
        5
    );
    assert_eq!((index, found), (99, 99));

    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 5.0, 0.0, -1.0, &mut index, &mut found) },
        1
    );
    assert_eq!((index, found), (99, 99));

    assert_eq!(
        unsafe {
            sm_hit_test(
                h.0,
                state.revision,
                5.0,
                0.0,
                0.0,
                ptr::null_mut(),
                &mut found,
            )
        },
        1
    );
    assert_eq!(found, 99);
}

#[test]
fn erase_rejects_invalid_state_and_revision_and_supports_undo() {
    let h = Handle::new();
    commit(h.0, 0.0);
    commit(h.0, 100.0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(unsafe { sm_erase_stroke(h.0, state.revision - 1, 0) }, 5);
    assert_eq!(unsafe { sm_erase_stroke(h.0, state.revision, 2) }, 1);

    assert_eq!(sm_begin(h.0, 2.0, 0), 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(unsafe { sm_erase_stroke(h.0, state.revision, 0) }, 3);
    assert_eq!(sm_cancel(h.0), 0);

    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(unsafe { sm_erase_stroke(h.0, state.revision, 0) }, 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.stroke_count, 1);

    let mut index = 99;
    let mut found = 99;
    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 5.0, 0.0, 0.0, &mut index, &mut found) },
        0
    );
    assert_eq!((index, found), (0, 0));

    assert_eq!(sm_undo(h.0), 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.stroke_count, 2);
    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 5.0, 0.0, 0.0, &mut index, &mut found) },
        0
    );
    assert_eq!((index, found), (0, 1));

    assert_eq!(sm_redo(h.0), 0);
    assert_eq!(unsafe { sm_get_state(h.0, &mut state) }, 0);
    assert_eq!(state.stroke_count, 1);
    assert_eq!(
        unsafe { sm_hit_test(h.0, state.revision, 5.0, 0.0, 0.0, &mut index, &mut found) },
        0
    );
    assert_eq!((index, found), (0, 0));
}
