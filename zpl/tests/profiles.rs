//! Specification command geometry vs. explicitly selected printer behavior.
//! References: Zebra Programming Guide ^FO p. 201, ^FT p. 205 (Table 7),
//! ^GD p. 213, ^BY/^BZ pp. 148/150, ^BA pp. 87–89; docs/printer-accuracy.md.
use zpl::render::profiles::SPECIFICATION;
use zpl::{
    output::raster::{rasterize, Raster},
    render,
    render::{compatibility::Compatibility, profiles::ZD621_203_DPI},
    Options,
};

fn raster(body: &str, options: Options) -> Raster {
    let source = format!("^XA^PW400^LL400{body}^FS^XZ");
    rasterize(&render(source.as_bytes(), options).unwrap().labels[0]).unwrap()
}

fn bounds(r: &Raster) -> (u32, u32, u32, u32) {
    let (mut x0, mut y0, mut x1, mut y1) = (r.width, r.height, 0, 0);
    for y in 0..r.height {
        for x in 0..r.width {
            if r.pixels[(y * r.width + x) as usize] == 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    assert!(x1 > x0 && y1 > y0);
    (x0, y0, x1, y1)
}

#[test]
fn profile_is_an_overridable_initial_value() {
    assert_eq!(Options::default(), ZD621_203_DPI);
    assert_eq!(SPECIFICATION.compatibility, Compatibility::default());
    assert_eq!(
        (ZD621_203_DPI.width, ZD621_203_DPI.height, ZD621_203_DPI.dpi),
        (832, 1218, 203)
    );
    let mut options = Options {
        width: 160,
        height: 140,
        dpi: 300,
        ..ZD621_203_DPI
    };
    options.compatibility.qr_fo_uses_by_height = false;
    let doc = render(b"^XA^BY2,3,60^FO20,30^BQN,2,2,L,0^FDLA,ABC^FS^XZ", options).unwrap();
    let scene = &doc.labels[0];
    assert_eq!((scene.width, scene.height, scene.dpi), (160, 140, 300));
    assert_eq!(bounds(&rasterize(scene).unwrap()).1, 30);
    let doc = render(b"^XA^PW120^LL110^XZ", options).unwrap();
    assert_eq!((doc.labels[0].width, doc.labels[0].height), (120, 110));
}

#[test]
fn qr_origins_have_independent_overrides() {
    for height in [40, 60, 100] {
        let fo = format!("^BY2,3,{height}^FO80,60^BQN,2,3,L,0^FDLA,HELLO123");
        assert_eq!(bounds(&raster(&fo, SPECIFICATION)).1, 60);
        assert_eq!(bounds(&raster(&fo, ZD621_203_DPI)).1, 60 + height - 1);
        let mut options = ZD621_203_DPI;
        options.compatibility.qr_fo_uses_by_height = false;
        assert_eq!(bounds(&raster(&fo, options)).1, 60);
    }
    for scale in 1..=5 {
        let ft = format!("^BY2,3,40^FT80,200^BQN,2,{scale},L,0^FDLA,HELLO123");
        assert_eq!(bounds(&raster(&ft, SPECIFICATION)).3, 200);
        assert_eq!(bounds(&raster(&ft, ZD621_203_DPI)).3, 200 - (3 * scale - 1));
        let mut options = ZD621_203_DPI;
        options.compatibility.qr_ft_includes_margin = false;
        assert_eq!(bounds(&raster(&ft, options)).3, 200);
    }
}

#[test]
fn diagonal_specification_stays_within_the_requested_box() {
    for (w, h) in [(120, 60), (40, 100), (15, 15)] {
        for t in [1, 3, 20] {
            for direction in ["L", "R"] {
                let body = format!("^FO80,80^GD{w},{h},{t},B,{direction}");
                let (x0, y0, x1, y1) = bounds(&raster(&body, SPECIFICATION));
                assert!(x0 >= 80 && y0 >= 80 && x1 <= 80 + w && y1 <= 80 + h);
                let mut options = ZD621_203_DPI;
                options.compatibility.diagonal_dot_runs = false;
                assert_eq!(raster(&body, options), raster(&body, SPECIFICATION));
            }
        }
    }
    assert_eq!(
        bounds(&raster("^FO80,80^GD120,60,3,B,L", ZD621_203_DPI)).2,
        203
    );
}

#[test]
fn postal_ratio_is_ignored_only_when_requested() {
    let label = |ratio| format!("^FO20,20^BY2,{ratio},80^BZN,80,N,N,0^FD12345");
    assert_ne!(
        raster(&label(2), SPECIFICATION),
        raster(&label(3), SPECIFICATION)
    );
    assert_eq!(
        raster(&label(2), ZD621_203_DPI),
        raster(&label(3), ZD621_203_DPI)
    );
    let mut options = ZD621_203_DPI;
    options.compatibility.postal_fixed_pitch = false;
    assert_eq!(raster(&label(2), options), raster(&label(2), SPECIFICATION));
}

#[test]
fn tracker_rounding_can_be_disabled_without_changing_pitch() {
    let body = "^FO20,20^BY2,2,80^BZN,80,N,N,3^FD0027012345620080000198765432101";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.intelligent_mail_outward_rounding = false;
    let fractional = raster(body, options);
    assert_ne!(printer, fractional);
    assert_eq!(bounds(&printer), bounds(&fractional));
    assert_eq!(printer.pixels.iter().filter(|&&p| p == 0).count(), 7176);
    assert_eq!(fractional.pixels.iter().filter(|&&p| p == 0).count(), 7052);
}

#[test]
fn retail_guard_specification_is_five_modules_and_override_is_in_dots() {
    // ISO/IEC 15420:2009, 4.3.3: guard and UPC-A outer-character extensions.
    // https://www.iso.org/standard/46143.html
    for module in [1, 2, 3] {
        let body = format!("^FO20,20^BY{module},2,80^BEN,80,N,N^FD590123412345");
        assert_eq!(bounds(&raster(&body, SPECIFICATION)).3, 100 + 5 * module);
        assert_eq!(bounds(&raster(&body, ZD621_203_DPI)).3, 113);
        let mut options = ZD621_203_DPI;
        options.compatibility.retail_guard_extension_dots = None;
        assert_eq!(raster(&body, options), raster(&body, SPECIFICATION));
        options.dpi = 300;
        options.compatibility.retail_guard_extension_dots = Some(7);
        assert_eq!(bounds(&raster(&body, options)).3, 107);
    }
}

#[test]
fn code93_normalization_is_explicit() {
    let source = b"^XA^FO20,20^BAN,80,N,N^FDHello93!^FS^XZ";
    assert!(render(source, SPECIFICATION)
        .unwrap_err()
        .message
        .contains("shift substitutes"));
    let mut options = ZD621_203_DPI;
    assert!(render(source, options).is_ok());
    options.compatibility.code93_normalize_input = false;
    assert!(render(source, options).is_err());
    let explicit = b"^XA^FO20,20^BAN,80,N,N^FD)H)E)L)L)O93(A^FS^XZ";
    assert!(render(explicit, options).is_ok());
}

#[test]
fn codablock_a_printer_options_are_independent() {
    let body = "^FO20,20^BY2,2,80^BBN,10,Y,10,2,A^FDABC123";
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)).3, 42);
    assert_eq!(bounds(&raster(body, SPECIFICATION)).3, 62);
    let mut options = ZD621_203_DPI;
    options.compatibility.codablock_a_row_height_in_dots = false;
    assert_eq!(raster(body, options), raster(body, SPECIFICATION));
    let long = "^FO20,20^BY2,2,80^BBN,10,Y,10,10,A^FDABCDEF";
    let mut specification = SPECIFICATION;
    specification.compatibility.codablock_a_row_height_in_dots = true;
    options = ZD621_203_DPI;
    options.compatibility.codablock_a_wrapping_checks = false;
    assert_eq!(raster(long, options), raster(long, specification));
    assert_ne!(raster(long, options), raster(long, ZD621_203_DPI));
}

#[test]
fn code128_above_text_can_keep_the_bar_origin() {
    let plain = raster("^FO80,80^BCN,60,N,N,N,N^FDABC123", ZD621_203_DPI);
    let above = "^FO80,80^BCN,60,Y,Y,N,N^FDABC123";
    let printer = raster(above, ZD621_203_DPI);
    for y in 80..140 {
        assert_eq!(
            &plain.pixels[y * 400..(y + 1) * 400],
            &printer.pixels[y * 400..(y + 1) * 400]
        );
    }
    let mut options = ZD621_203_DPI;
    options.compatibility.code128_above_text_keeps_bar_origin = false;
    let mut specification = SPECIFICATION;
    specification
        .compatibility
        .barcode_interpretation_printer_layout = true;
    assert_eq!(raster(above, options), raster(above, specification));
    assert_ne!(raster(above, options), printer);
}

#[test]
fn preview_layout_overrides_are_independent() {
    let plain = "^FO80,80^GB40,20,3";
    let top = "^LT20^FO80,80^GB40,20,3";
    let inverted = "^POI^FO80,80^GB40,20,3";
    assert_eq!(raster(plain, ZD621_203_DPI), raster(top, ZD621_203_DPI));
    assert_eq!(
        raster(plain, ZD621_203_DPI),
        raster(inverted, ZD621_203_DPI)
    );
    assert_eq!(bounds(&raster(top, SPECIFICATION)), (80, 100, 120, 120));
    assert_eq!(
        bounds(&raster(inverted, SPECIFICATION)),
        (280, 300, 320, 320)
    );
    let mut o = ZD621_203_DPI;
    o.compatibility.preview_ignores_label_top = false;
    assert_eq!(raster(top, o), raster(top, SPECIFICATION));
    o = ZD621_203_DPI;
    o.compatibility.preview_ignores_print_orientation = false;
    assert_eq!(raster(inverted, o), raster(inverted, SPECIFICATION));
}

#[test]
fn codablock_row_options_are_independent() {
    let source = b"^XA^PW832^LL400^FO60,60^BY2^BBN,10,Y,6,4,F^FDABCDEFGHIJKLMNOP^FS^XZ";
    let bounds_for = |o| bounds(&rasterize(&render(source, o).unwrap().labels[0]).unwrap());
    assert_eq!(bounds_for(ZD621_203_DPI).3, 92);
    let mut o = ZD621_203_DPI;
    o.compatibility.codablock_f_fit_rows = false;
    assert_eq!(bounds_for(o).3, 102);
    o.compatibility.codablock_f_row_height_in_dots = false;
    assert_eq!(bounds_for(o).3, 142);
    assert_eq!(bounds_for(o), bounds_for(SPECIFICATION));
    assert!(render(b"^XA^BBN,10,Y,6,2,F^FDABC^FS^XZ", ZD621_203_DPI).is_err());
}

#[test]
fn inverted_text_margin_is_an_independent_override() {
    let body = "^FO250,200,1^AAI,36,25^FDABC";
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)).2, 257);
    assert_eq!(bounds(&raster(body, SPECIFICATION)).2, 250);
    let mut o = ZD621_203_DPI;
    o.compatibility
        .right_justified_inverted_text_uses_ink_margin = false;
    assert_eq!(raster(body, o), raster(body, SPECIFICATION));
}

#[test]
fn caption_gap_override_preserves_explicit_font_and_bars() {
    let body = "^FO60,60^BY2^A0N,32,0^BCN,60,Y,N,N,N^FDABC123";
    let spec = raster(body, SPECIFICATION);
    let printer = raster(body, ZD621_203_DPI);
    assert_eq!(bounds(&printer).3, bounds(&spec).3 + 3);
    let mut o = ZD621_203_DPI;
    o.compatibility.barcode_interpretation_printer_layout = false;
    assert_eq!(raster(body, o), spec);
}

#[test]
fn composite_height_and_quiet_zone_are_independent() {
    // Zebra ^BR p. 135 specifies dots; captured ZD621 previews multiply by X.
    let body = "^FO60,60^BRN,11,2,1,30^FD0103212345678906|10ABC";
    let spec = bounds(&raster(body, SPECIFICATION));
    assert_eq!(spec, (60, 60, 350, 104));
    let mut options = SPECIFICATION;
    options.compatibility.composite_height_in_modules = true;
    assert_eq!(bounds(&raster(body, options)).3, 134);
    options.compatibility.composite_height_in_modules = false;
    options.compatibility.composite_linear_quiet_zone = true;
    let shifted = bounds(&raster(body, options));
    assert_eq!(shifted, (spec.0 + 20, spec.1, spec.2 + 20, spec.3));
}

#[test]
fn databar_retail_dimensions_and_upce_input_are_independent() {
    // ^BR p. 135: e applies to composite GS1-128, not the retail aliases.
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(
            raster("^FO60,60^BRN,9,2,1,20^FD590123412345", options),
            raster("^FO60,60^BRN,9,2,1,80^FD590123412345", options)
        );
    }
    let body = "^FO60,60^BRN,8,2,1,20^FD425261";
    let mut options = ZD621_203_DPI;
    options.compatibility.databar_upce_requires_upca_data = false;
    assert_eq!(bounds(&raster(body, options)), (74, 60, 176, 208));
    options.compatibility.databar_retail_printer_dimensions = false;
    assert_eq!(raster(body, options), raster(body, SPECIFICATION));
    assert_eq!(
        raster(body, SPECIFICATION),
        raster("^FO60,60^BRN,8,2,1,20^FD04210000526", SPECIFICATION)
    );
}

#[test]
fn aztec_binary_and_default_parity_choices_are_independent() {
    // ISO/IEC 24778 Table 2 permits either extended or split binary counts.
    let binary = format!("^FO60,60^BON,3^FH^FD{}", "_80".repeat(32));
    let printer = raster(&binary, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.aztec_preserve_binary_runs = false;
    assert_ne!(raster(&binary, options).pixels, printer.pixels);
    assert_eq!(bounds(&raster(&binary, options)), bounds(&printer));
    let small = format!("^FO60,60^BON,3^FH^FD{}", "_80".repeat(6));
    assert_eq!(bounds(&raster(&small, options)), (60, 60, 105, 105));
    options.compatibility.aztec_floor_default_error_correction = false;
    assert_eq!(bounds(&raster(&small, options)), (60, 60, 117, 117));
    // Explicit percentages meet their minimum; only the default is truncated.
    let explicit = format!("^FO60,60^BON,3,N,10^FH^FD{}", "_80".repeat(9));
    assert_eq!(raster(&explicit, options), raster(&explicit, ZD621_203_DPI));
}

#[test]
fn data_matrix_escape_and_edifact_choices_are_independent() {
    // ^BX pp. 145–147 have conflicting default-escape descriptions. Explicit
    // g is unambiguous and takes precedence over the captured firmware default.
    let body = "^FO60,60^BXN,3,200^FD_1ABC123";
    let explicit = "^FO60,60^BXN,3,200,0,0,6,_^FD_1ABC123";
    assert!(raster(body, SPECIFICATION) != raster(body, ZD621_203_DPI));
    assert!(raster(explicit, ZD621_203_DPI) == raster(body, SPECIFICATION));
    let mut options = ZD621_203_DPI;
    options.compatibility.data_matrix_default_tilde_escape = false;
    assert!(raster(body, options) == raster(body, SPECIFICATION));
    let edifact = "^FO60,60^BXN,3,200,0,0,6,_^FH^FD@ABC_5EDEF?GHI1234";
    assert!(raster(edifact, options) != raster(edifact, SPECIFICATION));
    options
        .compatibility
        .data_matrix_edifact_printer_transitions = false;
    assert!(raster(edifact, options) == raster(edifact, SPECIFICATION));
}

#[test]
fn maxicode_preview_departures_are_independent_options() {
    // ISO 16023:2000 Annex A/F and Zebra ^BD pp. 106–108. Captured
    // firmware behavior, including its undecodable mode-5 preview, is opt-in.
    let body = "^FO20,20^BD4^FDABCDEF";
    let printer = raster(body, ZD621_203_DPI);
    for change in [
        |c: &mut Compatibility| c.maxicode_terminal_latch = false,
        |c: &mut Compatibility| c.maxicode_printer_dot_geometry = false,
    ] {
        let mut options = ZD621_203_DPI;
        change(&mut options.compatibility);
        assert_ne!(printer.pixels, raster(body, options).pixels);
    }
    let short = "^FO20,20^BD4^FDABC";
    assert!(raster(short, ZD621_203_DPI)
        .pixels
        .iter()
        .all(|&v| v == 255));
    let mut options = ZD621_203_DPI;
    options.compatibility.maxicode_standard_minimum_six_bytes = false;
    assert!(raster(short, options).pixels.contains(&0));
    let nul = "^FO20,20^BD4^FH^FDABCDEF_00XYZ";
    assert_eq!(printer.pixels, raster(nul, ZD621_203_DPI).pixels);
    options = ZD621_203_DPI;
    options.compatibility.maxicode_nul_terminates_data = false;
    assert_ne!(printer.pixels, raster(nul, options).pixels);
    let mode5 = "^FO20,20^BD5^FDABCDEF";
    let fixed = raster(mode5, ZD621_203_DPI);
    assert_eq!(
        fixed.pixels,
        raster("^FO20,20^BD5^FD123456789", ZD621_203_DPI).pixels
    );
    options = ZD621_203_DPI;
    options.compatibility.maxicode_mode5_preview_omits_data = false;
    assert_ne!(fixed.pixels, raster(mode5, options).pixels);
}

#[test]
fn tlc39_preview_departures_are_independent_options() {
    // US20010045461A1 ¶0024–0029 and ¶0067/Fig. 2; Zebra ^BT pp. 140–141.
    // Geometry and the observed '*' separator can each be disabled separately.
    let body = "^FO20,40^BTN,2,2,40,2,4^FD239316,ABC,DEF";
    let printer = raster(body, ZD621_203_DPI);
    for change in [
        |c: &mut Compatibility| c.tlc39_asterisk_separator = false,
        |c: &mut Compatibility| c.tlc39_extended_link_flag = false,
        |c: &mut Compatibility| c.tlc39_printer_layout = false,
    ] {
        let mut options = ZD621_203_DPI;
        change(&mut options.compatibility);
        assert_ne!(printer.pixels, raster(body, options).pixels);
    }
    let mut options = ZD621_203_DPI;
    options.compatibility.tlc39_asterisk_separator = false;
    options.compatibility.tlc39_extended_link_flag = false;
    options.compatibility.tlc39_printer_layout = false;
    assert_eq!(
        raster(body, options).pixels,
        raster(body, SPECIFICATION).pixels
    );
    let unlinked = "^FO20,40^BTN,2,2,40,2,4^FD239316";
    assert_eq!(
        raster(unlinked, ZD621_203_DPI).pixels,
        raster(unlinked, SPECIFICATION).pixels
    );
}

#[test]
fn databar_expanded_no_date_preview_is_optional() {
    // ISO/IEC 24724:2011 §7.2.5.4.4, p. 28: a date sentinel of 38400
    // terminates the fixed-length symbol. The preview violates that rule.
    let mut base = ZD621_203_DPI;
    base.compatibility.databar_expanded_wide_bar_separator = false;
    let mut options = base;
    options.compatibility.databar_expanded_no_date_preview = false;
    for payload in [
        "01900123456789083103032768",
        "01900123456789083202010000",
        "01900123456789083203022768",
        "01900123456789083109099999",
    ] {
        for segments in [4, 22] {
            let body = format!("^FO20,20^BRN,6,1,1,80,{segments}^FD{payload}");
            let standard = raster(&body, SPECIFICATION);
            assert_ne!(raster(&body, base).pixels, standard.pixels);
            assert_eq!(raster(&body, options).pixels, standard.pixels);
        }
    }
    for payload in [
        "01900123456789083103032767", // Short-weight method remains valid.
        "0190012345678908310303276811250101", // An actual date remains valid.
    ] {
        let body = format!("^FO20,20^BRN,6,1,1,80,22^FD{payload}");
        assert_eq!(raster(&body, base).pixels, raster(&body, options).pixels);
    }
}

#[test]
fn databar_expanded_wide_bar_separator_is_optional() {
    // ISO/IEC 24724:2011 §7.2.8, p. 38; controls 3103-11-4 and gtin-4.
    // The row direction fix applies to both profiles. Only the measured
    // A1/B1 separator departure is selected by this compatibility option.
    let mut options = ZD621_203_DPI;
    options.compatibility.databar_expanded_wide_bar_separator = false;
    for (payload, added, removed) in [
        ("0190012345678908310301223311991231", 8, 0),
        ("0100012345678905", 20, 4),
    ] {
        let body = format!("^FO20,20^BRN,6,2,1,80,4^FD{payload}");
        let specification = raster(&body, SPECIFICATION);
        assert_eq!(raster(&body, options).pixels, specification.pixels);
        let printer = raster(&body, ZD621_203_DPI);
        let pairs: Vec<_> = printer.pixels.iter().zip(&specification.pixels).collect();
        assert_eq!(
            pairs.iter().filter(|(a, b)| **a == 0 && **b == 255).count(),
            added
        );
        assert_eq!(
            pairs.iter().filter(|(a, b)| **a == 255 && **b == 0).count(),
            removed
        );
    }
}
