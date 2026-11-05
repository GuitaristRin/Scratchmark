use scratchmark_core::{Error, Viewport};

#[test]
fn width_underflow_is_rejected() {
    // 参 spec/M3.md §页面坐标与视口坐标：合法正输入下溢为零时仍须报错。
    let tiny = Viewport {
        scale: f64::from_bits(1),
        ..Viewport::default()
    };
    assert_eq!(
        tiny.width_page_to_viewport(0.5),
        Err(Error::InvalidArgument)
    );
    let large = Viewport {
        scale: 2.0,
        ..Viewport::default()
    };
    assert_eq!(
        large.width_viewport_to_page(f64::from_bits(1)),
        Err(Error::InvalidArgument)
    );
    // 最小非零结果仍合法，不能一并拒绝可表示的次正规数。
    assert_eq!(tiny.width_page_to_viewport(1.0), Ok(f64::from_bits(1)));
    assert_eq!(
        large.width_viewport_to_page(f64::from_bits(2)),
        Ok(f64::from_bits(1))
    );
}
