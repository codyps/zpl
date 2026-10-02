//! Resource policies are configurable independently of ZPL specification semantics.
use zpl::{
    render::{profiles::SPECIFICATION, render_with_limits, Limits},
    Options,
};

fn options() -> Options {
    Options {
        width: 100,
        height: 100,
        ..SPECIFICATION
    }
}

#[test]
fn default_limits_preserve_scenes_and_diagnostics() {
    for input in [
        b"^XA^FO2,2^GB50,30,2^FS^XZ".as_slice(),
        b"^XA^XZ^XA^XZ",
        b"^XA^ZZ^XZ",
    ] {
        let ordinary = zpl::render(input, options());
        let limited = render_with_limits(input, options(), Limits::default());
        match (ordinary, limited) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.labels, b.labels);
                assert_eq!(a.warnings, b.warnings);
            }
            (Err(a), Err(b)) => {
                assert_eq!(a.offset, b.offset);
                assert_eq!(a.message, b.message);
            }
            _ => panic!("default limits changed rendering"),
        }
    }
}

#[test]
fn enforces_dimensions_labels_and_total_segments() {
    for input in [b"^XA^PW101^XZ".as_slice(), b"^XA^LL101^XZ"] {
        let error = render_with_limits(
            input,
            options(),
            Limits {
                dimension: 100,
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.message, "canvas exceeds configured renderer limit");
    }
    assert!(render_with_limits(
        b"^XA^XZ",
        options(),
        Limits {
            pixels: 9999,
            ..Limits::default()
        }
    )
    .is_err());
    let error = render_with_limits(
        b"^XA^XZ^XA^XZ",
        options(),
        Limits {
            labels: 1,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "too many labels (maximum 1)");
    let input = b"^XA^FO1,1^GB5,5,5^FS^XZ^XA^FO1,1^GB5,5,5^FS^XZ";
    let one = zpl::render(b"^XA^FO1,1^GB5,5,5^FS^XZ", options()).unwrap();
    let segments = one.labels[0].draws[0].path.segments.len();
    let error = render_with_limits(
        input,
        options(),
        Limits {
            segments,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "document path limit exceeded");
}

#[test]
fn limits_downloaded_graphics_and_expanded_formats() {
    let error = render_with_limits(
        b"~DGR:A.GRF,1,1,FF^XA^XGR:A.GRF,1,1^FS^XZ",
        options(),
        Limits {
            stored_graphic_segments: 0,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "graphic path limit exceeded");
    let input = b"^XA^DFR:A.ZPL^FO1,1^GB5,5,5^FS^FO2,2^GB5,5,5^FS^XZ^XA^XFR:A.ZPL^FS^XFR:A.ZPL^FS^XFR:A.ZPL^FS^XZ";
    let error = render_with_limits(
        input,
        options(),
        Limits {
            input_bytes: input.len(),
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "expanded formats exceed configured renderer limit"
    );
}

#[test]
fn callers_can_raise_input_label_and_canvas_limits() {
    let input = format!("^XA^FX{}^FS^XZ", "x".repeat(1_048_577));
    assert!(zpl::render(input.as_bytes(), options()).is_err());
    assert_eq!(
        render_with_limits(
            input.as_bytes(),
            options(),
            Limits {
                input_bytes: input.len(),
                ..Limits::default()
            }
        )
        .unwrap()
        .labels
        .len(),
        1
    );
    let input = b"^XA^XZ".repeat(65);
    assert!(zpl::render(&input, options()).is_err());
    assert_eq!(
        render_with_limits(
            &input,
            options(),
            Limits {
                labels: 65,
                ..Limits::default()
            }
        )
        .unwrap()
        .labels
        .len(),
        65
    );
    let input = b"^XA^PW1000001^LL34^FO1000000,0^GB1,1,1^FS^XZ";
    let document = render_with_limits(
        input,
        Options {
            height: 1,
            ..options()
        },
        Limits {
            pixels: 34_000_034,
            number_abs: 1_000_001.,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(document.labels[0].width, 1_000_001);
    assert_eq!(document.labels[0].draws.len(), 1);
}

#[test]
fn field_budget_covers_plain_numbered_and_concatenated_data() {
    for source in [
        format!("^XA^AAN,9,5^FD{}^FS^XZ", "A".repeat(4097)),
        format!("^XA^AAN,9,5^FN1^FD{}^FS^XZ", "A".repeat(4097)),
        format!(
            "^XA^AAN,9,5^FN1^FD{}^FS^FE#^FD#1##1#^FS^XZ",
            "A".repeat(2049)
        ),
    ] {
        assert!(zpl::render(source.as_bytes(), options()).is_err());
        assert!(render_with_limits(
            source.as_bytes(),
            options(),
            Limits {
                field_bytes: 4098,
                ..Limits::default()
            }
        )
        .is_ok());
    }
    let source = b"^XA^AAN,9,5^FDAB^FS^XZ";
    assert!(render_with_limits(
        source,
        options(),
        Limits {
            field_bytes: 1,
            ..Limits::default()
        }
    )
    .is_err());
}

#[test]
fn graphic_bytes_and_geometry_budgets_can_be_raised() {
    // 50,001 rows of AA contain four disjoint dots per row: 1,000,020 segments.
    let bitmap = "AA".repeat(50_001);
    let source = format!("~DGR:L.GRF,50001,1,{bitmap}^XA^XGR:L.GRF,1,1^FS^XZ");
    assert!(zpl::render(source.as_bytes(), options()).is_err());
    let raised = Limits {
        graphic_bytes: 50_001,
        segments: 1_000_020,
        stored_graphic_segments: 1_000_020,
        ..Limits::default()
    };
    let document = render_with_limits(source.as_bytes(), options(), raised).unwrap();
    assert_eq!(document.labels[0].draws[0].path.segments.len(), 1_000_020);
    for limits in [
        Limits {
            graphic_bytes: 50_000,
            ..raised
        },
        Limits {
            segments: 1_000_019,
            ..raised
        },
        Limits {
            stored_graphic_segments: 1_000_019,
            ..raised
        },
    ] {
        assert!(render_with_limits(source.as_bytes(), options(), limits).is_err());
    }
    // Exercise inline binary graphics through the separate parser path.
    let mut source = b"^XA^GFB,25001,25001,1,".to_vec();
    source.extend(vec![0; 25_001]);
    source.extend_from_slice(b"^FS^XZ");
    assert!(zpl::render(&source, options()).is_err());
    assert!(render_with_limits(&source, options(), Limits::unlimited()).is_ok());
}

#[test]
fn format_budgets_can_be_raised_without_allowing_cycles() {
    let mut source = "^XA^DFR:F0^FO1,1^GB2,2,2^FS^XZ".to_string();
    // Beyond both the old depth and object-count caps, with heap-based expansion.
    for i in 1..300 {
        source.push_str(&format!("^XA^DFR:F{i}^XFR:F{}^XZ", i - 1));
    }
    source.push_str("^XA^XFR:F299^XZ");
    assert!(zpl::render(source.as_bytes(), options()).is_err());
    let limits = Limits {
        stored_formats: 300,
        recall_depth: 300,
        recall_calls: 300,
        ..Limits::default()
    };
    let document = render_with_limits(source.as_bytes(), options(), limits).unwrap();
    assert_eq!(document.labels[0].draws.len(), 1);
    for limits in [
        Limits {
            stored_formats: 299,
            ..limits
        },
        Limits {
            recall_depth: 299,
            ..limits
        },
        Limits {
            recall_calls: 299,
            ..limits
        },
    ] {
        assert!(render_with_limits(source.as_bytes(), options(), limits).is_err());
    }
    let source = format!("^XA^DFT^XZ^XA{}^XZ", "^XFT".repeat(4097));
    assert!(zpl::render(source.as_bytes(), options()).is_err());
    assert!(render_with_limits(
        source.as_bytes(),
        options(),
        Limits {
            recall_calls: 4097,
            ..Limits::default()
        }
    )
    .is_ok());
    for source in [
        b"^XA^DFT^XFT^XZ^XA^XFT^XZ".as_slice(),
        b"^XA^DFA^XFB^XZ^XA^DFB^XFA^XZ^XA^XFA^XZ",
    ] {
        let error = render_with_limits(source, options(), Limits::unlimited()).unwrap_err();
        assert!(error.message.contains("recursive cycle"));
        assert_eq!(&source[error.offset..error.offset + 3], b"^XF");
    }
}

#[test]
fn expanded_byte_budget_is_checked_while_expanding() {
    let source = format!("^XA^DFT^FX{}^FS^XZ^XA^XFT^XFT^XZ", "x".repeat(600_000));
    assert!(zpl::render(source.as_bytes(), options()).is_err());
    assert!(render_with_limits(
        source.as_bytes(),
        options(),
        Limits {
            input_bytes: 1_300_000,
            ..Limits::default()
        }
    )
    .is_ok());
}

#[test]
fn unlimited_still_rejects_invalid_dimensions_and_nonfinite_coordinates() {
    for source in [
        b"^XA^PW4294967296^XZ".as_slice(),
        b"^XA^LL0^XZ",
        b"^XA^FOinf,0^GB1,1,1^FS^XZ",
        b"^XA^FONaN,0^GB1,1,1^FS^XZ",
    ] {
        assert!(render_with_limits(source, options(), Limits::unlimited()).is_err());
    }
    let options = Options {
        width: u32::MAX,
        compatibility: zpl::render::compatibility::Compatibility {
            preview_width_quantum: Some(2),
            ..SPECIFICATION.compatibility
        },
        ..options()
    };
    let error = render_with_limits(b"^XA^XZ", options, Limits::unlimited()).unwrap_err();
    assert_eq!(error.message, "rounded preview width overflows u32");
}

#[test]
fn coordinate_budget_covers_final_label_transforms() {
    let source = b"^XA^PW100^LL100^PMY^FO0,0^GB1,1,1^FS^XZ";
    let error = render_with_limits(
        source,
        options(),
        Limits {
            coordinate_abs: 50.,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "invalid path coordinate");
    assert!(render_with_limits(
        source,
        options(),
        Limits {
            coordinate_abs: 100.,
            ..Limits::default()
        }
    )
    .is_ok());
    let options = Options {
        width: 100,
        height: 100,
        compatibility: zpl::render::compatibility::Compatibility {
            preview_width_quantum: Some(8192),
            ..SPECIFICATION.compatibility
        },
        ..SPECIFICATION
    };
    let error = render_with_limits(
        b"^XA^FO0,0^GB1,1,1^FS^XZ",
        options,
        Limits {
            coordinate_abs: 50.,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.message, "invalid path coordinate");
    let document =
        render_with_limits(b"^XA^FO0,0^GB1,1,1^FS^XZ", options, Limits::unlimited()).unwrap();
    assert_eq!(document.labels[0].width, 8192);
}

#[test]
fn unlimited_graphic_header_cannot_overflow_decoded_size_arithmetic() {
    let source = format!("^XA^GFA,0,{},1,0^FS^XZ", usize::MAX / 2 + 1);
    let error = render_with_limits(source.as_bytes(), options(), Limits::unlimited()).unwrap_err();
    assert_eq!(error.message, "graphic nibble count overflow");
}
