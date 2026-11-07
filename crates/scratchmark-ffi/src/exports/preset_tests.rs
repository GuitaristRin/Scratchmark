use super::*;
use crate::sm_features;
use std::ptr;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn features_report_brush_preset() {
    assert_eq!(sm_features() & 2048, 2048);
}

#[test]
fn every_kind_and_level_is_queryable_through_the_c_abi() {
    // 笔型 1..5、档位 1..5 全部可查，且同一笔型内严格递增。
    for kind in 1..=5u32 {
        let mut previous = 0.0;
        for level in 1..=5u32 {
            let mut ratio = 0.0;
            assert_eq!(unsafe { sm_brush_preset(kind, level, &mut ratio) }, 0);
            assert!(ratio > 0.0 && ratio <= 1.0);
            assert!(ratio > previous, "kind {kind} level {level} 未递增");
            previous = ratio;
        }
    }
    // 手算锚点：铅笔（kind 1）第 3 档 0.26，笔（kind 4）第 5 档 0.62。
    let mut ratio = 0.0;
    assert_eq!(unsafe { sm_brush_preset(1, 3, &mut ratio) }, 0);
    close(ratio, 0.26);
    assert_eq!(unsafe { sm_brush_preset(4, 5, &mut ratio) }, 0);
    close(ratio, 0.62);
}

#[test]
fn width_conversion_matches_the_ratio() {
    let mut width = 0.0;
    // 铅笔第 3 档、最大宽度 10 → 2.6。
    assert_eq!(unsafe { sm_brush_preset_width(1, 3, 10.0, &mut width) }, 0);
    close(width, 2.6);
    // 最大宽度 1e6 仍是合法输入。
    assert_eq!(unsafe { sm_brush_preset_width(2, 5, 1e6, &mut width) }, 0);
    close(width, 750_000.0);
}

#[test]
fn invalid_inputs_are_rejected_without_writing_outputs() {
    let mut ratio = 99.0;
    let mut width = 99.0;

    for kind in [0u32, 6, u32::MAX] {
        assert_eq!(unsafe { sm_brush_preset(kind, 1, &mut ratio) }, 1);
        assert_eq!(
            unsafe { sm_brush_preset_width(kind, 1, 10.0, &mut width) },
            1
        );
    }
    for level in [0u32, 6] {
        assert_eq!(unsafe { sm_brush_preset(1, level, &mut ratio) }, 1);
        assert_eq!(
            unsafe { sm_brush_preset_width(1, level, 10.0, &mut width) },
            1
        );
    }
    for max_width in [0.0, -1.0, f64::NAN, 1e6 + 1.0] {
        assert_eq!(
            unsafe { sm_brush_preset_width(1, 1, max_width, &mut width) },
            1
        );
    }
    // 空指针。
    assert_eq!(unsafe { sm_brush_preset(1, 1, ptr::null_mut()) }, 1);
    assert_eq!(
        unsafe { sm_brush_preset_width(1, 1, 10.0, ptr::null_mut()) },
        1
    );
    assert_eq!((ratio, width), (99.0, 99.0));
}
