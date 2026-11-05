use scratchmark_core::{Bounds, BrushOptions, Document, Error, MAX_COORD, Point, Sample, Viewport};

fn vp(scale: f64, offset_x: f64, offset_y: f64) -> Viewport {
    Viewport {
        scale,
        offset_x,
        offset_y,
        reserved: 0,
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

// 浮点乘除不逐位可逆；断言用相对误差界而不是精确相等。
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn identity_viewport_maps_page_values_bit_exactly() {
    let identity = Viewport::default();
    for (x, y) in [(0.0, 0.0), (-1e9, 1e9), (0.1, -0.3), (1234.5678, 7.25e-8)] {
        assert_eq!(
            identity.page_to_viewport(Point { x, y }),
            Ok(Point { x, y })
        );
        assert_eq!(
            identity.viewport_to_page(Point { x, y }),
            Ok(Point { x, y })
        );
    }
    assert_eq!(identity.width_page_to_viewport(2.5), Ok(2.5));
    assert_eq!(identity.width_viewport_to_page(2.5), Ok(2.5));
}

#[test]
fn hand_calculated_forward_inverse_width_and_bounds() {
    let v = vp(2.0, 10.0, 20.0);
    assert_eq!(
        v.page_to_viewport(Point { x: 3.0, y: 4.0 }),
        Ok(Point { x: 16.0, y: 28.0 })
    );
    assert_eq!(
        v.viewport_to_page(Point { x: 16.0, y: 28.0 }),
        Ok(Point { x: 3.0, y: 4.0 })
    );
    let v = vp(1.5, -3.25, 0.0);
    assert_eq!(
        v.page_to_viewport(Point { x: 2.0, y: 7.0 }),
        Ok(Point { x: -0.25, y: 10.5 })
    );
    assert_eq!(v.width_page_to_viewport(5.0), Ok(7.5));
    assert_eq!(vp(2.0, 0.0, 0.0).width_viewport_to_page(10.0), Ok(5.0));
    let bounds = vp(2.0, 10.0, 20.0).bounds_page_to_viewport(Bounds {
        min_x: -1.0,
        min_y: -2.0,
        max_x: 3.0,
        max_y: 4.0,
    });
    assert_eq!(
        bounds,
        Ok(Bounds {
            min_x: 8.0,
            min_y: 16.0,
            max_x: 16.0,
            max_y: 28.0
        })
    );
    // 空笔划的全零边界也必须可映射。
    assert_eq!(
        vp(0.5, 7.0, 9.0).bounds_page_to_viewport(Bounds::default()),
        Ok(Bounds {
            min_x: 7.0,
            min_y: 9.0,
            max_x: 7.0,
            max_y: 9.0
        })
    );
}

#[test]
fn zoom_pan_roundtrip_stays_within_rounding_error() {
    for scale in [1e-6, 0.25, 1.0, 1.5, 4096.0, 1e6] {
        for (offset_x, offset_y) in [(0.0, 0.0), (-1e6, 1e6), (3.75, -4096.5)] {
            let v = vp(scale, offset_x, offset_y);
            for i in 0..=20usize {
                let x = -MAX_COORD + f64::from(i as u32) * 1e8;
                let y = MAX_COORD - f64::from(i as u32) * 1e8;
                let page = Point { x, y };
                let forward = v.page_to_viewport(page).unwrap();
                let back = v.viewport_to_page(forward).unwrap();
                assert!(
                    close(back.x, page.x) && close(back.y, page.y),
                    "scale {scale} offset ({offset_x},{offset_y})"
                );
                let round2 = v.page_to_viewport(back).unwrap();
                assert!(
                    close(round2.x, forward.x) && close(round2.y, forward.y),
                    "viewport side must agree with itself"
                );
            }
        }
    }
}

#[test]
fn input_path_and_display_path_produce_the_same_viewport_geometry() {
    let view = vp(1.5, 120.0, -48.0);
    let page_samples: Vec<_> = (0..40u32)
        .map(|i| {
            sample(
                f64::from(i) * 2.5,
                (f64::from(i) / 5.0).sin() * 30.0,
                u64::from(i),
            )
        })
        .collect();
    let options = BrushOptions {
        alpha: 0.5,
        min_width_ratio: 0.25,
        pressure_enabled: 0,
        reserved: 0,
    };

    let mut original = Document::default();
    original.begin(6.0, 0).unwrap();
    original.append(&page_samples).unwrap();
    let display: Vec<_> = {
        let revision = original.state().revision;
        let mut points = vec![scratchmark_core::BrushPoint::default(); page_samples.len()];
        original
            .read_brush_points(revision, 0, 0, options, &mut points)
            .unwrap();
        points
            .iter()
            .map(|p| view.page_to_viewport(Point { x: p.x, y: p.y }).unwrap())
            .collect()
    };

    // 输入路径：宿主把设备点逆变换回页面域再 append；样本保存的是逆变换结果。
    let appended: Vec<_> = page_samples
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let page = Point { x: p.x, y: p.y };
            let device = view.page_to_viewport(page).unwrap();
            let back = view.viewport_to_page(device).unwrap();
            sample(back.x, back.y, u64::try_from(i).unwrap())
        })
        .collect();
    let mut replayed = Document::default();
    replayed.begin(6.0, 0).unwrap();
    replayed.append(&appended).unwrap();
    let mut raw = vec![sample(0.0, 0.0, 0); appended.len()];
    let revision = replayed.state().revision;
    assert_eq!(
        replayed.read_points(revision, 0, 0, &mut raw).unwrap(),
        appended.len()
    );
    assert_eq!(raw, appended);
    let mut points = vec![scratchmark_core::BrushPoint::default(); appended.len()];
    replayed
        .read_brush_points(revision, 0, 0, options, &mut points)
        .unwrap();
    for (expected, point) in display.iter().zip(&points) {
        let shown = view
            .page_to_viewport(Point {
                x: point.x,
                y: point.y,
            })
            .unwrap();
        assert!(close(shown.x, expected.x) && close(shown.y, expected.y));
    }

    // 派生笔刷读出的页面几何经同一视口呈现，缩放只改变比例与位置。
    let zoomed = vp(view.scale * 2.0, view.offset_x, view.offset_y);
    for (before, point) in display.iter().zip(&points) {
        let after = zoomed
            .page_to_viewport(Point {
                x: point.x,
                y: point.y,
            })
            .unwrap();
        assert!(close(
            after.x - view.offset_x,
            2.0 * (before.x - view.offset_x)
        ));
    }
}

#[test]
fn hit_tolerance_converted_once_is_consistent_in_both_spaces() {
    let view = vp(0.75, 512.0, 256.0);
    let mut d = Document::default();
    // 页面里一条从 (0,0) 到 (100,0) 的水平笔划，固定宽 2 的中心线。
    d.begin(2.0, 0).unwrap();
    d.append(&[sample(0.0, 0.0, 0), sample(100.0, 0.0, 1)])
        .unwrap();
    d.finish().unwrap();
    let half_width_page = 1.0; // width / 2
    let tolerance_px = 12.0;
    let tolerance_page = view.width_viewport_to_page(tolerance_px).unwrap();
    let center_device = view.page_to_viewport(Point { x: 50.0, y: 0.0 }).unwrap();
    for probe_y in [-20.0, -17.1, -16.9, 0.0, 10.0, 16.9, 17.1, 20.0] {
        let probe = Point {
            x: 50.0,
            y: probe_y,
        };
        let device = view.page_to_viewport(probe).unwrap();
        // 页面空间判定：到中心线的距离 ≤ 半宽 + 容差。
        let page_hit = probe_y.abs() <= half_width_page + tolerance_page;
        // 视口空间判定：同一换算的距离不等式。
        let device_hit = (device.y - center_device.y).abs()
            <= view.width_page_to_viewport(half_width_page).unwrap() + tolerance_px;
        assert_eq!(page_hit, device_hit, "probe y = {probe_y}");
    }
}

#[test]
fn invalid_viewports_points_and_widths_are_rejected_without_output() {
    let v = vp(2.0, 10.0, 20.0);
    let in_page = Point { x: 1.0, y: 2.0 };
    let non_finite = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    for value in non_finite {
        assert_eq!(vp(1.0, value, 0.0).validate(), Err(Error::InvalidArgument));
        assert_eq!(vp(1.0, 0.0, value).validate(), Err(Error::InvalidArgument));
    }
    // scale 必须有限且 > 0；0 / 负数 / 非有限都拒绝，offset 为 0 合法。
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        assert_eq!(vp(value, 0.0, 0.0).validate(), Err(Error::InvalidArgument));
    }
    assert_eq!(
        vp(1.0, 0.0, 0.0).page_to_viewport(in_page),
        Ok(Point { x: 1.0, y: 2.0 })
    );
    let mut reserved = vp(1.0, 0.0, 0.0);
    reserved.reserved = 1;
    assert_eq!(reserved.validate(), Err(Error::InvalidArgument));
    // 页面域输入越界拒绝；视口输入只要求有限，大视口值映射回域内是合法的。
    for value in [f64::NAN, f64::INFINITY, MAX_COORD + 1.0, -MAX_COORD - 1.0] {
        assert_eq!(
            v.page_to_viewport(Point { x: value, y: 0.0 }),
            Err(Error::InvalidArgument)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            v.viewport_to_page(Point { x: value, y: 0.0 }),
            Err(Error::InvalidArgument)
        );
    }
    assert_eq!(
        v.viewport_to_page(Point {
            x: MAX_COORD + 1.0,
            y: 0.0,
        }),
        Ok(Point {
            x: (MAX_COORD + 1.0 - 10.0) / 2.0,
            y: -10.0,
        })
    );
    // 逆映射结果必须落回页面域，缩得过小时大量视口值都越界。
    let tiny = vp(1e-9, 0.0, 0.0);
    assert_eq!(
        tiny.viewport_to_page(Point { x: 5e9, y: 0.0 }),
        Err(Error::InvalidArgument)
    );
    // 溢出为非有限一律拒绝，不静默截断。
    assert_eq!(
        vp(1e300, 0.0, 0.0).page_to_viewport(Point { x: 1e9, y: 0.0 }),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        vp(f64::from_bits(1), 0.0, 0.0).viewport_to_page(Point { x: 1.0, y: 0.0 }),
        Err(Error::InvalidArgument)
    );
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(v.width_page_to_viewport(value), Err(Error::InvalidArgument));
        assert_eq!(v.width_viewport_to_page(value), Err(Error::InvalidArgument));
    }
    assert_eq!(
        vp(1e300, 0.0, 0.0).width_page_to_viewport(1e9),
        Err(Error::InvalidArgument)
    );
    // 有效值不受无效调用影响（错误路径为纯函数，无状态可污染）。
    assert_eq!(v.page_to_viewport(in_page), Ok(Point { x: 12.0, y: 24.0 }));
}
