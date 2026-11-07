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

fn sample(x: f64, y: f64, time_us: u64) -> Sample {
    Sample {
        x,
        y,
        time_us,
        pressure: 0.0,
        flags: 0,
    }
}

fn line(h: u64, from: (f64, f64), to: (f64, f64), width: f64) {
    assert_eq!(sm_begin(h, width, 0x11223344), 0);
    let points = [sample(from.0, from.1, 0), sample(to.0, to.1, 10)];
    assert_eq!(unsafe { sm_append(h, points.as_ptr(), 2) }, 0);
    assert_eq!(sm_finish(h), 0);
}

fn state(h: u64) -> State {
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(h, &mut state) }, 0);
    state
}

#[test]
fn features_report_page_extent() {
    assert_eq!(sm_features() & 1024, 1024);
}

#[test]
fn document_bounds_reports_empty_and_union() {
    let h = Handle::new();

    // 空文档：found=0 且边界全零。
    let mut bounds = Bounds {
        min_x: 9.0,
        min_y: 9.0,
        max_x: 9.0,
        max_y: 9.0,
    };
    let mut strokes = 99;
    let mut found = 99;
    assert_eq!(
        unsafe {
            sm_document_bounds(
                h.0,
                state(h.0).revision,
                &mut bounds,
                &mut strokes,
                &mut found,
            )
        },
        0
    );
    assert_eq!((strokes, found), (0, 0));
    assert_eq!(
        (bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
        (0.0, 0.0, 0.0, 0.0)
    );

    // 两笔并集，含笔宽。
    line(h.0, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(h.0, (100.0, 50.0), (120.0, 60.0), 4.0);
    let revision = state(h.0).revision;
    assert_eq!(
        unsafe { sm_document_bounds(h.0, revision, &mut bounds, &mut strokes, &mut found) },
        0
    );
    assert_eq!((strokes, found), (2, 1));
    assert_eq!((bounds.min_x, bounds.min_y), (-1.0, -1.0));
    assert_eq!((bounds.max_x, bounds.max_y), (122.0, 62.0));

    // 过期 revision 与空指针不写输出。
    let mut keep = Bounds {
        min_x: 7.0,
        min_y: 7.0,
        max_x: 7.0,
        max_y: 7.0,
    };
    let mut keep_found = 77;
    assert_eq!(
        unsafe { sm_document_bounds(h.0, revision + 1, &mut keep, &mut strokes, &mut keep_found,) },
        5
    );
    assert_eq!((keep.min_x, keep_found), (7.0, 77));
    assert_eq!(
        unsafe {
            sm_document_bounds(
                h.0,
                revision,
                ptr::null_mut(),
                &mut strokes,
                &mut keep_found,
            )
        },
        1
    );
    assert_eq!(keep_found, 77);
}

#[test]
fn translate_all_moves_the_whole_document_in_one_undo() {
    let h = Handle::new();
    line(h.0, (0.0, 0.0), (10.0, 0.0), 2.0);
    line(h.0, (0.0, 100.0), (10.0, 100.0), 2.0);
    let revision = state(h.0).revision;

    assert_eq!(unsafe { sm_translate_all(h.0, revision, 0.0, 50.0) }, 0);
    let after = state(h.0);
    assert_eq!(after.revision, revision + 1);
    assert_eq!(after.stroke_count, 2);
    let mut bounds = Bounds::default();
    let mut strokes = 0;
    let mut found = 0;
    assert_eq!(
        unsafe { sm_document_bounds(h.0, after.revision, &mut bounds, &mut strokes, &mut found,) },
        0
    );
    assert_eq!((bounds.min_y, bounds.max_y), (49.0, 151.0));

    // 一次撤销整体回退。
    assert_eq!(sm_undo(h.0), 0);
    let reverted = state(h.0).revision;
    assert_eq!(
        unsafe { sm_document_bounds(h.0, reverted, &mut bounds, &mut strokes, &mut found) },
        0
    );
    assert_eq!((bounds.min_y, bounds.max_y), (-1.0, 101.0));
}

#[test]
fn translate_all_validates_and_is_atomic() {
    let h = Handle::new();
    let revision = state(h.0).revision;
    line(h.0, (0.0, 0.0), (10.0, 0.0), 2.0);

    // 非法参数、过期 revision、越界。
    assert_eq!(
        unsafe { sm_translate_all(h.0, state(h.0).revision, f64::NAN, 0.0) },
        1
    );
    assert_eq!(unsafe { sm_translate_all(h.0, revision + 9, 1.0, 1.0) }, 5);
    assert_eq!(
        unsafe { sm_translate_all(h.0, state(h.0).revision, 1e9, 0.0) },
        1
    );

    let mut bounds = Bounds::default();
    let mut strokes = 0;
    let mut found = 0;
    assert_eq!(
        unsafe {
            sm_document_bounds(
                h.0,
                state(h.0).revision,
                &mut bounds,
                &mut strokes,
                &mut found,
            )
        },
        0
    );
    assert_eq!((bounds.min_x, found), (-1.0, 1));
    assert_eq!(unsafe { sm_translate_all(0, revision, 1.0, 1.0) }, 2);
}
