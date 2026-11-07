use scratchmark_core::{Bounds, Error, Point, Viewport, ViewportEx};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn bounds(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Bounds {
    Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn identity(angle: f64) -> ViewportEx {
    ViewportEx {
        scale: 1.0,
        angle,
        offset_x: 0.0,
        offset_y: 0.0,
        reserved: 0,
    }
}

#[test]
fn zero_angle_matches_the_old_viewport_exactly() {
    let legacy = Viewport {
        scale: 2.5,
        offset_x: 13.0,
        offset_y: -7.0,
        reserved: 0,
    };
    let extended = ViewportEx {
        scale: 2.5,
        angle: 0.0,
        offset_x: 13.0,
        offset_y: -7.0,
        reserved: 0,
    };
    for (x, y) in [(0.0, 0.0), (120.0, -40.0), (-800.0, 900.0)] {
        let legacy_point = legacy.page_to_viewport(point(x, y)).unwrap();
        let extended_point = extended.page_to_viewport(point(x, y)).unwrap();
        assert_eq!(legacy_point.x, extended_point.x);
        assert_eq!(legacy_point.y, extended_point.y);
    }
    let legacy_bounds = legacy
        .bounds_page_to_viewport(bounds(-1.0, -2.0, 3.0, 4.0))
        .unwrap();
    let extended_bounds = extended
        .bounds_page_to_viewport(bounds(-1.0, -2.0, 3.0, 4.0))
        .unwrap();
    assert_eq!(legacy_bounds, extended_bounds);
}

#[test]
fn quarter_turn_rotates_axes_and_round_trips() {
    let viewport = identity(FRAC_PI_2);
    // 90°：(x, y) -> (−y, x)
    let mapped = viewport.page_to_viewport(point(10.0, 0.0)).unwrap();
    close(mapped.x, 0.0);
    close(mapped.y, 10.0);

    let back = viewport.viewport_to_page(point(0.0, 10.0)).unwrap();
    close(back.x, 10.0);
    close(back.y, 0.0);

    for angle in [0.0, FRAC_PI_2, PI, 0.7, -1.9, TAU + 0.3] {
        let viewport = ViewportEx {
            scale: 1.7,
            angle,
            offset_x: 40.0,
            offset_y: -25.0,
            reserved: 0,
        };
        for (x, y) in [(0.0, 0.0), (300.0, -120.0), (-45.0, 620.0)] {
            let view = viewport.page_to_viewport(point(x, y)).unwrap();
            let page = viewport.viewport_to_page(view).unwrap();
            close(page.x, x);
            close(page.y, y);
        }
    }
}

#[test]
fn rotated_bounds_are_the_axis_aligned_circumscribed_box() {
    // 90°：宽高互换。
    let viewport = identity(FRAC_PI_2);
    let boxed = viewport
        .bounds_page_to_viewport(bounds(0.0, 0.0, 100.0, 20.0))
        .unwrap();
    close(boxed.min_x, -20.0);
    close(boxed.max_x, 0.0);
    close(boxed.min_y, 0.0);
    close(boxed.max_y, 100.0);

    // 45°：正方形外接框按 √2 放大。
    let viewport = identity(PI / 4.0);
    let boxed = viewport
        .bounds_page_to_viewport(bounds(0.0, 0.0, 10.0, 10.0))
        .unwrap();
    close(boxed.max_x - boxed.min_x, 10.0 * 2f64.sqrt());
    close(boxed.max_y - boxed.min_y, 10.0 * 2f64.sqrt());
}

#[test]
fn width_only_depends_on_scale() {
    let flat = identity(0.0);
    let turned = ViewportEx {
        angle: PI / 3.0,
        scale: 2.0,
        ..identity(0.0)
    };
    close(flat.width_page_to_viewport(6.0).unwrap(), 6.0);
    close(turned.width_page_to_viewport(6.0).unwrap(), 12.0);
    close(turned.width_viewport_to_page(12.0).unwrap(), 6.0);
    assert_eq!(
        turned.width_page_to_viewport(0.0),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn invalid_viewports_and_points_are_rejected() {
    const NAN: f64 = f64::NAN;
    const INFINITY: f64 = f64::INFINITY;
    let good = identity(0.0);
    let bad = [
        ViewportEx { scale: 0.0, ..good },
        ViewportEx {
            scale: -1.0,
            ..good
        },
        ViewportEx { scale: NAN, ..good },
        ViewportEx { angle: NAN, ..good },
        ViewportEx {
            angle: INFINITY,
            ..good
        },
        ViewportEx {
            offset_x: NAN,
            ..good
        },
        ViewportEx {
            reserved: 1,
            ..good
        },
    ];
    for viewport in bad {
        assert_eq!(
            viewport.page_to_viewport(point(0.0, 0.0)),
            Err(Error::InvalidArgument)
        );
        assert_eq!(
            viewport.bounds_page_to_viewport(bounds(0.0, 0.0, 1.0, 1.0)),
            Err(Error::InvalidArgument)
        );
    }
    // 页面域越界与视口输出越界。
    assert_eq!(
        good.page_to_viewport(point(1e9 + 1.0, 0.0)),
        Err(Error::InvalidArgument)
    );
    // 视口输出只要求有限：极大缩放仍是合法映射。
    let huge = ViewportEx { scale: 1e9, ..good };
    assert!(huge.page_to_viewport(point(1e9, 1e9)).is_ok());
    // 视口坐标反算落到页面域外。
    let tiny = ViewportEx {
        scale: 1e-12,
        ..good
    };
    assert_eq!(
        tiny.viewport_to_page(point(1e6, 0.0)),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn zoom_keeps_the_page_point_under_the_anchor() {
    for angle in [0.0, FRAC_PI_2, 0.6, -2.2] {
        let viewport = ViewportEx {
            scale: 1.3,
            angle,
            offset_x: 12.0,
            offset_y: -8.0,
            reserved: 0,
        };
        let anchor = point(240.0, 180.0);
        let before = viewport.viewport_to_page(anchor).unwrap();
        let zoomed = viewport.zoom_at(anchor, 1.8).unwrap();
        let after = zoomed.viewport_to_page(anchor).unwrap();
        close(after.x, before.x);
        close(after.y, before.y);
        close(zoomed.scale, 1.3 * 1.8);
        close(zoomed.angle, angle);

        // 两次缩放等价于一次合成缩放。
        let twice = zoomed.zoom_at(anchor, 0.5).unwrap();
        let once = viewport.zoom_at(anchor, 1.8 * 0.5).unwrap();
        close(twice.scale, once.scale);
        close(twice.offset_x, once.offset_x);
        close(twice.offset_y, once.offset_y);
    }

    let viewport = identity(0.0);
    assert_eq!(
        viewport.zoom_at(point(0.0, 0.0), 0.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        viewport.zoom_at(point(0.0, 0.0), -1.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        viewport.zoom_at(point(f64::NAN, 0.0), 2.0),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn fit_centers_the_page_and_respects_padding() {
    let page = bounds(0.0, 0.0, 800.0, 600.0);
    for angle in [0.0, FRAC_PI_2, 0.4] {
        let fitted = ViewportEx::fit(page, 1000.0, 800.0, angle, 20.0).unwrap();
        let view = fitted.bounds_page_to_viewport(page).unwrap();
        // 落在视口内，且至少一个方向刚好用满可用区域。
        assert!(view.min_x >= 20.0 - 1e-6 && view.max_x <= 980.0 + 1e-6);
        assert!(view.min_y >= 20.0 - 1e-6 && view.max_y <= 780.0 + 1e-6);
        close((view.min_x + view.max_x) / 2.0, 500.0);
        close((view.min_y + view.max_y) / 2.0, 400.0);
    }

    // 90° 时页面外接框变成 600×800，缩放由高度（含内边距）决定。
    let fitted = ViewportEx::fit(page, 1000.0, 800.0, FRAC_PI_2, 20.0).unwrap();
    close(fitted.scale, 760.0 / 800.0);

    // 退化页面与过大内边距被拒绝。
    assert_eq!(
        ViewportEx::fit(bounds(5.0, 5.0, 5.0, 5.0), 100.0, 100.0, 0.0, 0.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        ViewportEx::fit(page, 100.0, 100.0, 0.0, 60.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        ViewportEx::fit(page, 0.0, 100.0, 0.0, 0.0),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        ViewportEx::fit(bounds(100.0, 0.0, 0.0, 10.0), 100.0, 100.0, 0.0, 0.0),
        Err(Error::InvalidArgument)
    );
}
