use scratchmark_core::{BRUSH_LEVELS, BrushKind, Error};

const KINDS: [BrushKind; 5] = [
    BrushKind::Pencil,
    BrushKind::Marker,
    BrushKind::Ballpen,
    BrushKind::Pen,
    BrushKind::Eraser,
];

#[test]
fn every_kind_and_level_returns_a_monotone_ratio() {
    assert_eq!(BRUSH_LEVELS, 5);
    for kind in KINDS {
        let mut previous = 0.0;
        for level in 1..=BRUSH_LEVELS {
            let ratio = kind.ratio(level).unwrap();
            assert!(
                ratio > 0.0 && ratio <= 1.0,
                "{kind:?} level {level}: {ratio}"
            );
            assert!(
                ratio > previous,
                "{kind:?} 档位必须严格递增：level {level} = {ratio}，上一档 {previous}"
            );
            previous = ratio;
        }
    }
}

#[test]
fn width_scales_with_the_requested_maximum() {
    for kind in KINDS {
        for level in 1..=BRUSH_LEVELS {
            let ratio = kind.ratio(level).unwrap();
            for max_width in [1.0, 10.0, 1e6] {
                let width = kind.width(level, max_width).unwrap();
                assert_eq!(width, max_width * ratio);
                assert!(width > 0.0 && width <= 1e6);
            }
        }
    }
    // 具体手算：铅笔第 3 档在最大宽度 10 时为 2.6。
    assert_eq!(BrushKind::Pencil.width(3, 10.0).unwrap(), 2.6);
}

#[test]
fn invalid_kind_level_and_width_are_rejected() {
    for kind in [0u32, 6, 99, u32::MAX] {
        assert_eq!(BrushKind::from_u32(kind), Err(Error::InvalidArgument));
    }
    for level in [0u32, 6, 42] {
        assert_eq!(BrushKind::Pencil.ratio(level), Err(Error::InvalidArgument));
        assert_eq!(
            BrushKind::Pencil.width(level, 10.0),
            Err(Error::InvalidArgument)
        );
    }
    for max_width in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e6 + 1.0] {
        assert_eq!(
            BrushKind::Pencil.width(1, max_width),
            Err(Error::InvalidArgument)
        );
    }
}

#[test]
fn kinds_are_distinguishable() {
    // 不同笔型在同一档位给出不同比例（否则这张表就没有意义）。
    let ratios: Vec<f64> = KINDS.iter().map(|kind| kind.ratio(5).unwrap()).collect();
    for (index, left) in ratios.iter().enumerate() {
        for right in &ratios[index + 1..] {
            assert!(
                (left - right).abs() > 1e-9,
                "档位 5 的比例不应重复：{ratios:?}"
            );
        }
    }
}
