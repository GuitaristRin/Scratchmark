use scratchmark_core::{BrushOptions, BrushPoint, Document, Error, Sample};

fn options() -> BrushOptions {
    BrushOptions {
        alpha: 0.5,
        min_width_ratio: 0.25,
        pressure_enabled: 1,
        reserved: 0,
    }
}
fn samples() -> [Sample; 4] {
    [
        Sample {
            x: 0.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.0,
            flags: 1,
        },
        Sample {
            x: 8.0,
            y: 4.0,
            time_us: 1,
            pressure: 0.5,
            flags: 1,
        },
        Sample {
            x: 8.0,
            y: 4.0,
            time_us: 2,
            pressure: 1.0,
            flags: 1,
        },
        Sample {
            x: 4.0,
            y: 0.0,
            time_us: 3,
            pressure: 0.0,
            flags: 0,
        },
    ]
}
fn read(d: &mut Document, options: BrushOptions) -> Vec<BrushPoint> {
    let state = d.state();
    let info = d.stroke_info(state.revision, 0).unwrap();
    let mut points = vec![BrushPoint::default(); info.point_count as usize];
    let n = d
        .read_brush_points(state.revision, 0, 0, options, &mut points)
        .unwrap();
    assert_eq!(n, points.len());
    points
}

#[test]
fn hand_calculated_smoothing_and_pressure_preserve_raw_samples() {
    let mut d = Document::default();
    d.begin(10.0, 0).unwrap();
    d.append(&samples()).unwrap();
    assert_eq!(
        read(&mut d, options()),
        vec![
            BrushPoint {
                x: 0.0,
                y: 0.0,
                width: 2.5
            },
            BrushPoint {
                x: 4.0,
                y: 2.0,
                width: 6.25
            },
            BrushPoint {
                x: 6.0,
                y: 3.0,
                width: 10.0
            },
            BrushPoint {
                x: 5.0,
                y: 1.5,
                width: 10.0
            },
        ]
    );
    let revision = d.state().revision;
    let mut raw = samples();
    d.read_points(revision, 0, 0, &mut raw).unwrap();
    assert_eq!(raw, samples());
    let identity = read(&mut d, BrushOptions::default());
    for (actual, raw) in identity.iter().zip(samples()) {
        assert_eq!(
            *actual,
            BrushPoint {
                x: raw.x,
                y: raw.y,
                width: 10.0
            }
        );
    }
    assert_eq!(d.state().revision, revision);
}

#[test]
fn batch_and_read_partitions_do_not_change_cached_geometry() {
    let mut all = Document::default();
    all.begin(10.0, 0).unwrap();
    all.append(&samples()).unwrap();
    let expected = read(&mut all, options());
    for cut in 1..=4 {
        let mut d = Document::default();
        d.begin(10.0, 0).unwrap();
        d.append(&samples()[..cut]).unwrap();
        assert_eq!(read(&mut d, options()), expected[..cut]);
        let old = d.state().revision;
        let bad = Sample {
            x: f64::NAN,
            ..samples()[0]
        };
        assert_eq!(d.append(&[samples()[3], bad]), Err(Error::InvalidArgument));
        assert_eq!(d.state().revision, old);
        assert_eq!(read(&mut d, options()), expected[..cut]);
        d.append(&samples()[cut..]).unwrap();
        let revision = d.state().revision;
        for (i, point) in expected.iter().enumerate() {
            let mut one = [BrushPoint::default()];
            assert_eq!(
                d.read_brush_points(revision, 0, i as u32, options(), &mut one),
                Ok(1)
            );
            assert_eq!(one[0], *point);
        }
        d.finish().unwrap();
        assert_eq!(read(&mut d, options()), expected);
        d.undo().unwrap();
        d.redo().unwrap();
        assert_eq!(read(&mut d, options()), expected);
    }
}

#[test]
fn cached_views_do_not_change_the_shared_snapshot_or_restore_results() {
    let fixture: Vec<u8> = include_str!("../../../spec/fixtures/simple-v1.hex")
        .split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect();
    let mut d = Document::default();
    d.load(&fixture).unwrap();
    let expected = read(&mut d, options());
    assert_eq!(
        expected,
        vec![
            BrushPoint {
                x: 0.0,
                y: 1.0,
                width: 2.0
            },
            BrushPoint {
                x: 1.0,
                y: 0.0,
                width: 1.25
            }
        ]
    );
    assert_eq!(d.save().unwrap(), fixture);
    let mut restored = Document::default();
    restored.load(&d.save().unwrap()).unwrap();
    assert_eq!(read(&mut restored, options()), expected);
    restored.begin(3.0, 0).unwrap();
    restored.append(&samples()).unwrap();
    restored.cancel().unwrap();
    assert_eq!(restored.save().unwrap(), fixture);
}

#[test]
fn invalid_options_and_stale_reads_leave_output_and_state_untouched() {
    let mut d = Document::default();
    d.begin(10.0, 0).unwrap();
    d.append(&samples()).unwrap();
    let revision = d.state().revision;
    let sentinel = BrushPoint {
        x: 99.0,
        y: 98.0,
        width: 97.0,
    };
    let mut invalid = vec![
        BrushOptions {
            pressure_enabled: 2,
            ..options()
        },
        BrushOptions {
            reserved: 1,
            ..options()
        },
    ];
    for value in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1.01] {
        invalid.push(BrushOptions {
            alpha: value,
            ..options()
        });
        invalid.push(BrushOptions {
            min_width_ratio: value,
            ..options()
        });
    }
    for option in invalid {
        let mut output = [sentinel];
        assert_eq!(
            d.read_brush_points(revision, 0, 0, option, &mut output),
            Err(Error::InvalidArgument)
        );
        assert_eq!(output, [sentinel]);
        assert_eq!(d.state().revision, revision);
    }
    let mut output = [sentinel];
    assert_eq!(
        d.read_brush_points(revision - 1, 0, 0, options(), &mut output),
        Err(Error::StaleRevision)
    );
    assert_eq!(
        d.read_brush_points(revision, 0, 5, options(), &mut output),
        Err(Error::InvalidArgument)
    );
    assert_eq!(output, [sentinel]);
    assert_eq!(
        d.read_brush_points(revision, 0, 4, options(), &mut output),
        Ok(0)
    );
    assert_eq!(
        d.read_brush_points(revision, 0, 0, options(), &mut []),
        Ok(0)
    );
}

#[test]
fn missing_pressure_single_dots_and_underflow_have_finite_positive_widths() {
    for width in [f64::from_bits(1), 1.0, 1e6] {
        let mut d = Document::default();
        d.begin(width, 0).unwrap();
        d.append(&samples()[..1]).unwrap();
        let tiny = BrushOptions {
            min_width_ratio: f64::from_bits(1),
            ..options()
        };
        let output = read(&mut d, tiny);
        assert_eq!((output[0].x, output[0].y), (0.0, 0.0));
        assert!(output[0].width > 0.0 && output[0].width <= width);
        d.append(&samples()[3..]).unwrap();
        assert_eq!(read(&mut d, tiny)[1].width, width);
    }
}

#[test]
fn derived_centerline_and_discs_stay_inside_reported_bounds() {
    let mut d = Document::default();
    d.begin(10.0, 0).unwrap();
    let points: Vec<_> = (0..400)
        .map(|i| Sample {
            x: f64::from(i) / 10.0,
            y: (f64::from(i) / 7.0).sin(),
            time_us: i as u64,
            pressure: (i % 101) as f32 / 100.0,
            flags: 1,
        })
        .collect();
    d.append(&points).unwrap();
    let bounds = d.stroke_info(d.state().revision, 0).unwrap();
    for p in read(&mut d, options()) {
        assert!(p.x - p.width / 2.0 >= bounds.min_x && p.x + p.width / 2.0 <= bounds.max_x);
        assert!(p.y - p.width / 2.0 >= bounds.min_y && p.y + p.width / 2.0 <= bounds.max_y);
    }
}
