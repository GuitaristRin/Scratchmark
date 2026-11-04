use super::*;
use crate::{sm_begin, sm_destroy, sm_features};
use std::ptr;

#[test]
fn brush_buffers_and_invalid_options_preserve_scalar_and_array_outputs() {
    assert_eq!(sm_features() & 1, 1);
    let mut handle = 0;
    assert_eq!(unsafe { sm_create(&mut handle) }, 0);
    assert_eq!(sm_begin(handle, 10.0, 0), 0);
    let p = Sample {
        x: 2.0,
        y: 4.0,
        time_us: 0,
        pressure: 0.5,
        flags: 1,
    };
    assert_eq!(unsafe { sm_append(handle, &p, 1) }, 0);
    let mut state = State::default();
    assert_eq!(unsafe { sm_get_state(handle, &mut state) }, 0);
    let mut options = BrushOptions {
        alpha: 0.5,
        min_width_ratio: 0.25,
        pressure_enabled: 1,
        reserved: 0,
    };
    let sentinel = BrushPoint {
        x: 99.0,
        y: 98.0,
        width: 97.0,
    };
    let mut point = sentinel;
    let mut written = 99;
    unsafe {
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision,
                0,
                0,
                ptr::null(),
                &mut point,
                1,
                &mut written
            ),
            1
        );
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision,
                0,
                0,
                &options,
                &mut point,
                1,
                ptr::null_mut()
            ),
            1
        );
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision - 1,
                0,
                0,
                &options,
                &mut point,
                1,
                &mut written
            ),
            5
        );
        options.alpha = f64::NAN;
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision,
                0,
                0,
                &options,
                &mut point,
                1,
                &mut written
            ),
            1
        );
        assert_eq!((point, written), (sentinel, 99));
        options.alpha = 0.5;
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision,
                0,
                0,
                &options,
                ptr::null_mut(),
                0,
                &mut written
            ),
            0
        );
        assert_eq!(written, 0);
        assert_eq!(
            sm_read_brush_points(
                handle,
                state.revision,
                0,
                0,
                &options,
                &mut point,
                1,
                &mut written
            ),
            0
        );
        assert_eq!(
            point,
            BrushPoint {
                x: 2.0,
                y: 4.0,
                width: 6.25
            }
        );
    }
    assert_eq!(sm_destroy(handle), 0);
}
