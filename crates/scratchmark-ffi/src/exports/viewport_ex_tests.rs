use super::*;
use crate::{sm_destroy, sm_features};
use std::f64::consts::FRAC_PI_2;
use std::ptr;

fn identity(angle: f64) -> ViewportEx {
    ViewportEx {
        scale: 1.0,
        angle,
        offset_x: 0.0,
        offset_y: 0.0,
        reserved: 0,
    }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn features_report_viewport_rotation() {
    assert_eq!(sm_features() & 128, 128);
}

#[test]
fn rotation_transforms_round_trip_through_the_c_abi() {
    let viewport = ViewportEx {
        scale: 2.0,
        angle: FRAC_PI_2,
        offset_x: 10.0,
        offset_y: -5.0,
        reserved: 0,
    };
    let page = Point { x: 10.0, y: 0.0 };
    let mut view = Point::default();
    assert_eq!(
        unsafe { sm_page_to_viewport_ex(&viewport, &page, &mut view) },
        0
    );
    // 90°：(10,0) -> (0,10)，再乘 2 并平移。
    close(view.x, 10.0);
    close(view.y, 15.0);

    let mut back = Point::default();
    assert_eq!(
        unsafe { sm_viewport_to_page_ex(&viewport, &view, &mut back) },
        0
    );
    close(back.x, 10.0);
    close(back.y, 0.0);

    let boxed = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 20.0,
    };
    let mut rotated = Bounds::default();
    assert_eq!(
        unsafe { sm_bounds_page_to_viewport_ex(&viewport, &boxed, &mut rotated) },
        0
    );
    // 90° 后 100×20 变成 20×100，按 2 倍缩放并平移。
    close(rotated.min_x, -30.0);
    close(rotated.max_x, 10.0);
    close(rotated.min_y, -5.0);
    close(rotated.max_y, 195.0);

    let mut width = 0.0;
    assert_eq!(
        unsafe { sm_width_page_to_viewport_ex(&viewport, 6.0, &mut width) },
        0
    );
    close(width, 12.0);
    assert_eq!(
        unsafe { sm_width_viewport_to_page_ex(&viewport, 12.0, &mut width) },
        0
    );
    close(width, 6.0);
}

#[test]
fn zoom_and_fit_are_exposed_and_atomic() {
    let viewport = identity(0.6);
    let anchor = Point { x: 240.0, y: 180.0 };
    let before = viewport.viewport_to_page(anchor).unwrap();
    let mut zoomed = ViewportEx::default();
    assert_eq!(
        unsafe { sm_viewport_ex_zoom_at(&viewport, &anchor, 1.75, &mut zoomed) },
        0
    );
    let after = zoomed.viewport_to_page(anchor).unwrap();
    close(after.x, before.x);
    close(after.y, before.y);
    close(zoomed.scale, 1.75);

    // 非法 factor 不写输出。
    let mut untouched = ViewportEx {
        scale: 9.0,
        ..ViewportEx::default()
    };
    assert_eq!(
        unsafe { sm_viewport_ex_zoom_at(&viewport, &anchor, 0.0, &mut untouched) },
        1
    );
    assert_eq!(untouched.scale, 9.0);
    assert_eq!(
        unsafe { sm_viewport_ex_zoom_at(&viewport, ptr::null(), 1.0, &mut untouched) },
        1
    );

    let page = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 800.0,
        max_y: 600.0,
    };
    let mut fitted = ViewportEx::default();
    assert_eq!(
        unsafe { sm_viewport_ex_fit(&page, 1000.0, 800.0, 0.0, 20.0, &mut fitted) },
        0
    );
    // 0°：960/800 = 1.2，760/600 ≈ 1.267 → 取 1.2。
    close(fitted.scale, 1.2);
    // 页面中心映射到视口中心。
    let center = fitted
        .page_to_viewport(Point { x: 400.0, y: 300.0 })
        .unwrap();
    close(center.x, 500.0);
    close(center.y, 400.0);

    // 过大的内边距不写输出。
    let mut keep = ViewportEx {
        scale: 7.0,
        ..ViewportEx::default()
    };
    assert_eq!(
        unsafe { sm_viewport_ex_fit(&page, 100.0, 100.0, 0.0, 60.0, &mut keep) },
        1
    );
    assert_eq!(keep.scale, 7.0);
    assert_eq!(
        unsafe { sm_viewport_ex_fit(ptr::null(), 100.0, 100.0, 0.0, 0.0, &mut keep) },
        1
    );
}

#[test]
fn invalid_viewport_structures_are_rejected() {
    let page = Point { x: 1.0, y: 2.0 };
    let mut view = Point::default();
    for bad in [
        ViewportEx {
            scale: 0.0,
            ..identity(0.0)
        },
        ViewportEx {
            angle: f64::NAN,
            ..identity(0.0)
        },
        ViewportEx {
            reserved: 1,
            ..identity(0.0)
        },
    ] {
        assert_eq!(unsafe { sm_page_to_viewport_ex(&bad, &page, &mut view) }, 1);
    }
    assert_eq!(view.x, 0.0);
    // 句柄无关的纯函数不接受 NULL。
    assert_eq!(
        unsafe { sm_page_to_viewport_ex(ptr::null(), &page, &mut view) },
        1
    );
    assert_eq!(sm_destroy(0), 2);
}
