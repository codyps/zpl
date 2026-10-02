//! Service budgets supplement the existing limits without changing default rendering.
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
fn callers_cannot_raise_renderer_limits() {
    let limits = Limits {
        input_bytes: usize::MAX,
        labels: usize::MAX,
        segments: usize::MAX,
        stored_graphic_segments: usize::MAX,
        pixels: usize::MAX,
        dimension: u32::MAX,
    };
    let error = render_with_limits(&vec![b' '; 1_048_577], options(), limits).unwrap_err();
    assert_eq!(error.message, "input exceeds 1 MiB renderer limit");
    let error = render_with_limits(&b"^XA^XZ".repeat(65), options(), limits).unwrap_err();
    assert_eq!(error.message, "too many labels (maximum 64)");
}
