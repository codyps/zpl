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
fn retail_caption_and_baseline_overrides_are_independent() {
    // ^BU pp. 142–143, ^FT p. 205; raw controls in retail-caption-zd621-v1.
    let body = "^BY2,3,60^FO80,80^BUN,70,Y,N^FD01234567890";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.retail_interpretation_printer_layout = false;
    let generic = raster(body, options);
    assert_ne!(printer.pixels, generic.pixels);
    assert_eq!(
        &printer.pixels[..150 * 400],
        &generic.pixels[..150 * 400],
        "caption layout must preserve the bars"
    );
    assert_eq!(
        printer.pixels,
        raster(&body.replace("^BUN", "^A0N,32,32^BUN"), ZD621_203_DPI).pixels
    );
    let body = "^BY2,3,60^FT80,80^B8N,1,N,N^FD1234567";
    options = ZD621_203_DPI;
    options.compatibility.linear_barcode_ft_uses_last_bar_row = false;
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)).1, 80);
    assert_eq!(bounds(&raster(body, options)).1, 79);
}

#[test]
fn postal_baseline_uses_the_parsed_variant() {
    // ^BZ numeric t parameter, pp. 150–153: equivalent spellings must
    // select the same rendering and ^FT boundary policy.
    for kind in [0, 1] {
        let body = format!("^BY2,2,20^FT80,80^BZN,20,N,N,{kind}^FD12345678901");
        let padded = body.replace(&format!(",{kind}^FD"), &format!(",0{kind}^FD"));
        assert_eq!(
            raster(&body, ZD621_203_DPI).pixels,
            raster(&padded, ZD621_203_DPI).pixels
        );
    }
}

#[test]
fn linear_caption_overrides_preserve_bar_geometry() {
    // Captured Code 11/93 delimiters, Codabar letters, and postal pitch:
    // linear-caption-zd621-v1; ^B1/^BA/^BK/^BZ command descriptions.
    for (body, option) in [
        ("^B1N,N,40,Y,N^FD98765", 0),
        ("^BAN,40,Y,N^FDABC123", 1),
        ("^BKN,N,40,Y,N,C,D^FD98765", 2),
        ("^B5N,40,Y,N^FD12345678901", 3),
        ("^A0N,32,32^B2N,40,Y,N,N^FD12345678", 4),
    ] {
        let body = format!("^BY2,2,40^FO40,80{body}");
        let printer = raster(&body, ZD621_203_DPI);
        let mut options = ZD621_203_DPI;
        match option {
            0 => options.compatibility.code11_interpretation_symbols = false,
            1 => options.compatibility.code93_interpretation_symbols = false,
            2 => options.compatibility.codabar_interpretation_delimiters = false,
            3 => options.compatibility.postal_interpretation_full_pitch = false,
            _ => options.compatibility.linear_interpretation_ignores_font = false,
        }
        let without = raster(&body, options);
        assert_ne!(printer.pixels, without.pixels);
        assert_eq!(&printer.pixels[..120 * 400], &without.pixels[..120 * 400]);
    }
    let body = "^BY3,2,40^FO40,80^B1N,Y,40,N,N^FD09-";
    let mut options = ZD621_203_DPI;
    options.compatibility.code11_printer_element_widths = false;
    assert_ne!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
}

#[test]
fn code93_extended_checksum_preview_is_independent_of_encoding() {
    // ^BA e (p. 88), code93-checks-zd621-v1: extended C values enter
    // the ZD621 shift formatter; the encoded C/K bars remain unchanged.
    let body = "^BY2,2,40^FO40,80^BAN,40,Y,N,Y^FDAN";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.code93_extended_checksum_preview = false;
    let literal = raster(body, options);
    assert_ne!(printer.pixels, literal.pixels);
    assert_eq!(&printer.pixels[..120 * 400], &literal.pixels[..120 * 400]);
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for data in ["ABC123", "AN", "AO", "AP", "AQ"] {
            let hidden = format!("^BY2,2,40^FO40,80^BAN,40,N,N,Y^FD{data}");
            assert_eq!(
                raster(&hidden, options).pixels,
                raster(&hidden.replace(",N,N,Y^FD", ",N,N,N^FD"), options).pixels,
                "checksum interpretation must not change bars when hidden"
            );
        }
    }
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
fn code39_wide_elements_follow_the_selected_rounding_policy() {
    // ^BY p. 148 explicitly rounds module 9, ratio 2.4 from 21.6 to 22
    // dots. Raw ZD621 captures instead use 21 dots. *A* contains nine wide
    // elements and twenty narrow elements, including the intercharacter gaps.
    let body = "^BY9,2.4,20^FO10,10^B3N,N,20,N,N^FDA";
    let spec = raster(body, SPECIFICATION);
    let printer = raster(body, ZD621_203_DPI);
    let s = bounds(&spec);
    let p = bounds(&printer);
    assert_eq!(s.2 - s.0, 20 * 9 + 9 * 22);
    assert_eq!(p.2 - p.0, 20 * 9 + 9 * 21);
    let mut options = SPECIFICATION;
    options.compatibility.code39_floor_wide_elements = true;
    assert_eq!(raster(body, options).pixels, printer.pixels);
    options = ZD621_203_DPI;
    options.compatibility.code39_floor_wide_elements = false;
    assert_eq!(raster(body, options).pixels, spec.pixels);
}

#[test]
fn barcode_interpretation_origin_overrides_are_independent() {
    // ^FO p. 201 and ^B2 pp. 66–69; measured above/below controls in
    // barcode-interpretation-zd621-v1. Text must not move the bar origin.
    for (rotation, expected) in [
        ('N', (80, 54, 242, 150)),
        ('R', (80, 80, 176, 242)),
        ('I', (80, 80, 242, 176)),
        ('B', (54, 80, 150, 242)),
    ] {
        let body = format!("^BY2,3,60^FO80,80^B2{rotation},70,Y,Y^FD12345678");
        assert_eq!(bounds(&raster(&body, ZD621_203_DPI)), expected);
    }
    let mut options = ZD621_203_DPI;
    options.compatibility.barcode_above_text_keeps_bar_origin = false;
    let normal = "^BY2,3,60^FO80,80^B2N,70,Y,Y^FD12345678";
    assert_eq!(bounds(&raster(normal, options)), (80, 80, 242, 176));
    options = ZD621_203_DPI;
    options.compatibility.barcode_fo_uses_bar_height = false;
    let rotated = "^BY2,3,60^FO80,80^B2R,70,Y,Y^FD12345678";
    assert_eq!(bounds(&raster(rotated, options)), (106, 80, 202, 242));
    options = ZD621_203_DPI;
    options.compatibility.barcode_reverse_interpretation_shift = false;
    let inverted = "^BY2,3,60^FO80,80^B2I,70,Y,Y^FD12345678";
    assert_ne!(
        raster(inverted, options).pixels,
        raster(inverted, ZD621_203_DPI).pixels
    );
}

#[test]
fn code39_caption_and_baseline_overrides_are_independent() {
    // Raw Code 39 caption controls: checksum, explicit fonts, and ^FT's
    // one-dot-high boundary. ^B3 pp. 70–72 and ^FT p. 205, Table 7.
    let body = "^BY2,3,60^FO80,80^B3N,Y,70,Y,Y^FDABC123";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.code39_interpretation_symbols = false;
    let data_only = raster(body, options);
    assert_ne!(printer.pixels, data_only.pixels);
    // The caption override cannot alter the encoded bar rows.
    assert_eq!(
        printer.pixels[80 * 400..150 * 400],
        data_only.pixels[80 * 400..150 * 400]
    );
    let explicit = "^BY2,3,60^FO80,80^A0N,32,32^B3N,Y,70,Y,Y^FDABC123";
    assert_eq!(raster(explicit, ZD621_203_DPI).pixels, printer.pixels);
    options = ZD621_203_DPI;
    options.compatibility.code39_interpretation_ignores_font = false;
    assert_ne!(raster(explicit, options).pixels, printer.pixels);
    let one_row = "^BY2,3,60^FT80,80^B3N,N,1,N,N^FDABC123";
    assert_eq!(bounds(&raster(one_row, ZD621_203_DPI)).1, 80);
    options = ZD621_203_DPI;
    options.compatibility.linear_barcode_ft_uses_last_bar_row = false;
    assert_eq!(bounds(&raster(one_row, options)).1, 79);
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
    options.compatibility.tlc39_additional_data_byte_capacity = false;
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
fn barcode_defaults_follow_power_up_and_retained_by_values() {
    // Zebra Programming Guide ^BY, p. 148. Both profiles use documented
    // defaults; hardware comparisons are in barcode-defaults-zd621-v1.
    for options in [SPECIFICATION, ZD621_203_DPI] {
        let field = "^FO20,40^B3N,N,,N,N^FD123456";
        assert_eq!(
            raster(field, options).pixels,
            raster(&format!("^BY2,3,10{field}"), options).pixels
        );
        assert_eq!(
            raster(&format!("^BY3,2,64^BY{field}"), options).pixels,
            raster(&format!("^BY3,2,64{field}"), options).pixels
        );
        assert_eq!(
            raster(&format!("^BY3,2,64^BY,,10{field}"), options).pixels,
            raster(&format!("^BY3,2,10{field}"), options).pixels
        );
    }
}

#[test]
fn tlc39_additional_field_capacity_is_optional() {
    // ZD621 length-2-12 reserves Byte capacity despite emitting Text words.
    let multi = "^FO20,40^BTN,2,2,40,2,4^FD239316,AAAAAAAAAAAA,BBBBBBBBBBBB";
    let single = "^FO20,40^BTN,2,2,40,2,4^FD239316,AAAAAAAAAAAA";
    let mut options = ZD621_203_DPI;
    options.compatibility.tlc39_additional_data_byte_capacity = false;
    assert_ne!(
        raster(multi, options).pixels,
        raster(multi, ZD621_203_DPI).pixels
    );
    assert_eq!(
        raster(single, options).pixels,
        raster(single, ZD621_203_DPI).pixels
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

#[test]
fn rounded_box_inner_geometry_is_optional() {
    // Zebra ^GB pp. 210–211 and shapes-zd621-v1: the captured border floor
    // and independent inner rounding percentage are printer choices.
    let body = "^FO20,20^GB100,60,4,B,4";
    let mut options = ZD621_203_DPI;
    options.compatibility.rounded_box_printer_geometry = false;
    options.compatibility.rounded_box_printer_curve = false;
    assert_eq!(
        raster(body, options).pixels,
        raster(body, SPECIFICATION).pixels
    );
    assert_ne!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
    let thin = "^FO20,20^GB60,40,1,B,4";
    let two = "^FO20,20^GB60,40,2,B,4";
    assert_eq!(
        raster(thin, ZD621_203_DPI).pixels,
        raster(two, ZD621_203_DPI).pixels
    );
    assert_ne!(
        raster(thin, SPECIFICATION).pixels,
        raster(two, SPECIFICATION).pixels
    );
    let square = "^FO20,20^GB60,40,1,B,0";
    assert_eq!(
        raster(square, ZD621_203_DPI).pixels,
        raster(square, SPECIFICATION).pixels
    );
}

#[test]
fn rounded_box_curve_is_independently_optional() {
    // Captured integer corner recurrence: rounded-boxes-zd621-v1.
    let body = "^FO20,20^GB100,60,4,B,4";
    let mut options = ZD621_203_DPI;
    options.compatibility.rounded_box_printer_curve = false;
    assert_ne!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
    options.compatibility.rounded_box_printer_curve = true;
    options.compatibility.rounded_box_printer_geometry = false;
    assert_ne!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
}

#[test]
fn circle_printer_scan_conversion_is_optional() {
    // Zebra ^GC, Programming Guide pp. 212–213, specifies nominal diameter
    // and border. circles-zd621-v1 records the preview's different dot spans.
    for body in [
        "^FO20,20^GC21,1,B",
        "^FO20,20^GC80,3,B",
        "^FO20,20^GC32,100,B",
        "^FO20,20^GE32,32,3,B",
    ] {
        let mut options = ZD621_203_DPI;
        options.compatibility.circle_printer_curve = false;
        assert_eq!(
            raster(body, options).pixels,
            raster(body, SPECIFICATION).pixels
        );
        assert_ne!(
            raster(body, options).pixels,
            raster(body, ZD621_203_DPI).pixels
        );
    }
}

#[test]
fn unequal_axis_ellipse_curve_is_independently_optional() {
    // ^GE dimensions and border: Zebra Programming Guide p. 214. The measured
    // scan conversion and remaining residuals are pinned in ellipses-zd621-v1.
    for body in ["^FO20,20^GE120,60,3,B", "^FO20,20^GE60,120,3,B"] {
        let mut options = ZD621_203_DPI;
        options.compatibility.ellipse_printer_curve = false;
        assert_eq!(
            raster(body, options).pixels,
            raster(body, SPECIFICATION).pixels
        );
        assert_ne!(
            raster(body, options).pixels,
            raster(body, ZD621_203_DPI).pixels
        );
        options.compatibility.ellipse_printer_curve = true;
        options.compatibility.circle_printer_curve = false;
        assert_eq!(
            raster(body, options).pixels,
            raster(body, ZD621_203_DPI).pixels
        );
    }
}

#[test]
fn barcode_negative_ink_clamping_is_optional() {
    // Raw ZD621 controls: barcode-edges-zd621-v1; ^FO/^FT pp. 201/205.
    let body = "^BY3,2,80^FO60,10^B1N,Y,80,Y,Y^FD123-45678";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.linear_barcode_clamps_negative_ink = false;
    assert_ne!(printer.pixels, raster(body, options).pixels);
    let inset = "^BY3,2,80^FO60,60^B1N,Y,80,Y,Y^FD123-45678";
    assert_eq!(
        raster(inset, ZD621_203_DPI).pixels,
        raster(inset, options).pixels
    );
}

#[test]
fn rotated_bar_edge_trimming_is_independent_of_ink_clamping() {
    // Raw hardware controls: barcode-boundary-zd621-v1; ^FO/^FT pp. 201/205.
    let zero = "^BY1,2,30^FO0,40^BCR,30,N,N,N,N^FDCode128";
    let mut untrimmed = ZD621_203_DPI;
    untrimmed
        .compatibility
        .linear_barcode_rotated_edge_loses_dot = false;
    assert_ne!(
        raster(zero, ZD621_203_DPI).pixels,
        raster(zero, untrimmed).pixels
    );
    let mut trim_only = ZD621_203_DPI;
    trim_only.compatibility.linear_barcode_clamps_negative_ink = false;
    assert_eq!(
        raster(zero, ZD621_203_DPI).pixels,
        raster(zero, trim_only).pixels
    );
    let caption = "^BY1,2,30^FO0,40^BCR,30,Y,Y,N,N^FDCode128";
    assert_eq!(
        raster(caption, ZD621_203_DPI).pixels,
        raster(caption, trim_only).pixels
    );
    let inset = "^BY1,2,30^FO1,40^BCR,30,N,N,N,N^FDCode128";
    assert_eq!(
        raster(inset, ZD621_203_DPI).pixels,
        raster(inset, untrimmed).pixels
    );
}

#[test]
fn caption_clamping_preserves_rotated_short_glyph_padding() {
    // barcode-padding-zd621-v1: at x=16 the dash ink is already visible,
    // but its seven-row resident-A area crosses the edge. ^FO p. 201.
    let body = "^BY2,2,1^FO16,40^BCR,1,Y,N,N,N^FD----";
    let mut unclamped = ZD621_203_DPI;
    unclamped.compatibility.linear_barcode_clamps_negative_ink = false;
    assert_ne!(
        raster(body, ZD621_203_DPI).pixels,
        raster(body, unclamped).pixels
    );
}

#[test]
fn bitmap_ft_dot_origin_is_optional_and_leaves_fo_unchanged() {
    // ZPL Guide ^FT p. 205; resident-bc-zd621-v1 independent scale controls.
    for (font, h, w) in [
        ('A', 18, 10),
        ('B', 22, 14),
        ('C', 36, 20),
        ('D', 36, 20),
        ('E', 56, 30),
        ('F', 52, 26),
        ('G', 120, 80),
        ('H', 42, 26),
    ] {
        let body = format!("^FT100,100^A{font}N,{h},{w}^FDAb09");
        let mut geometric = ZD621_203_DPI;
        geometric.compatibility.bitmap_font_ft_dot_origin = false;
        assert_ne!(
            raster(&body, ZD621_203_DPI).pixels,
            raster(&body, geometric).pixels
        );
        let fo = body.replace("^FT", "^FO");
        assert_eq!(
            raster(&fo, ZD621_203_DPI).pixels,
            raster(&fo, geometric).pixels
        );
    }
}

#[test]
fn bitmap_cf_font_only_reset_is_optional() {
    // ^CF p. 154 says omitted sizes retain the last CF values; the ZD621
    // controls in resident-bc-zd621-v1 instead reset an explicitly selected font.
    let body = "^CFB,33,14^CFB^FO20,20^FDAb09";
    let mut retained = ZD621_203_DPI;
    retained.compatibility.bitmap_cf_font_only_resets_size = false;
    assert_ne!(
        raster(body, ZD621_203_DPI).pixels,
        raster(body, retained).pixels
    );
    assert_eq!(
        raster(body, retained).pixels,
        raster("^CFB,33,14^FO20,20^FDAb09", retained).pixels
    );
    assert_eq!(
        raster(body, ZD621_203_DPI).pixels,
        raster("^CFB,11,7^FO20,20^FDAb09", retained).pixels
    );
}

#[test]
fn bitmap_sizes_use_the_supplied_axis_or_both_cf_dimensions() {
    // ^A pp. 60–61, ^CF p. 154; hardware controls include these B/C cases.
    for (font, h, w) in [('B', 11, 7), ('C', 18, 10)] {
        let width_only = format!("^CF0,32,0^FO20,20^A{font}N,0,{}^FDAb09", w * 2);
        let explicit = format!("^FO20,20^A{font}N,{},{}^FDAb09", h * 2, w * 2);
        assert_eq!(
            raster(&width_only, SPECIFICATION).pixels,
            raster(&explicit, SPECIFICATION).pixels
        );
        let inherit = format!("^CF{font},{},{}^FO20,20^A{font}N,0,0^FDAb09", h * 3, w * 2);
        let explicit = format!("^FO20,20^A{font}N,{},{}^FDAb09", h * 3, w * 2);
        assert_eq!(
            raster(&inherit, SPECIFICATION).pixels,
            raster(&explicit, SPECIFICATION).pixels
        );
    }
}

#[test]
fn explicit_bitmap_code128_caption_shift_is_optional() {
    // ^BC p. 94; resident-f-zd621-v1 measures all orientations and A/B/C/D/F.
    let mut unshifted = ZD621_203_DPI;
    unshifted.compatibility.barcode_reverse_interpretation_shift = false;
    for (font, h, w) in [
        ('A', 18, 10),
        ('B', 22, 14),
        ('C', 36, 20),
        ('D', 36, 20),
        ('E', 56, 30),
        ('F', 52, 26),
        ('G', 120, 80),
        ('H', 42, 26),
        ('0', 32, 64),
    ] {
        for rotation in ['N', 'R', 'I', 'B'] {
            let body =
                format!("^BY2,2,60^FT200,200^A{font}N,{h},{w}^BC{rotation},60,Y,N,N,N^FDAb09");
            let same = raster(&body, ZD621_203_DPI).pixels == raster(&body, unshifted).pixels;
            assert_eq!(same, font == '0' || matches!(rotation, 'N' | 'R'), "{body}");
        }
    }
}

#[test]
fn code128_bar_width_pivot_is_independent_of_caption_shift() {
    // ^FO p. 201; wide-caption A/F controls in resident-f-zd621-v1.
    let mut full_extent = ZD621_203_DPI;
    full_extent.compatibility.code128_fo_uses_bar_width = false;
    for rotation in ['N', 'R', 'I', 'B'] {
        let body = format!("^BY2,2,60^FO100,100^AFN,78,39^BC{rotation},60,Y,N,N,N^FDAb09");
        assert_eq!(
            raster(&body, ZD621_203_DPI).pixels == raster(&body, full_extent).pixels,
            matches!(rotation, 'N' | 'R')
        );
        let ft = body.replace("^FO100,100", "^FT200,200");
        assert_eq!(
            raster(&ft, ZD621_203_DPI).pixels,
            raster(&ft, full_extent).pixels
        );
        let hidden = body.replace(",Y,N,N,N", ",N,N,N,N");
        assert_eq!(
            raster(&hidden, ZD621_203_DPI).pixels,
            raster(&hidden, full_extent).pixels
        );
    }
}

#[test]
fn ocr_b_inverted_margin_is_scaled_and_optional() {
    // ^FO p. 201; resident-e-zd621-v1 varies width and the trailing glyph.
    let mut geometric = ZD621_203_DPI;
    geometric
        .compatibility
        .right_justified_inverted_text_uses_ink_margin = false;
    for scale in [1, 2, 3] {
        let body = format!("^FO200,100,1^AEI,28,{}^FDAb09", 15 * scale);
        assert_eq!(bounds(&raster(&body, ZD621_203_DPI)).2, 200 + 6 * scale + 2);
        assert_eq!(bounds(&raster(&body, geometric)).2, 200);
    }
}

#[test]
fn resident_g_font_only_cf_reset_is_optional() {
    // ^CF p. 154; resident-g-zd621-v1 pins inherited and reset dimensions.
    let body = "^CFG,180,80^CFG^FO20,20^FDAb";
    let mut retained = ZD621_203_DPI;
    retained.compatibility.bitmap_cf_font_only_resets_size = false;
    assert_eq!(
        raster(body, retained),
        raster("^CFG,180,80^FO20,20^FDAb", retained)
    );
    assert_eq!(
        raster(body, ZD621_203_DPI),
        raster("^CFG,60,40^FO20,20^FDAb", retained)
    );
    assert_ne!(raster(body, retained), raster(body, ZD621_203_DPI));
}

#[test]
fn graphic_symbol_baseline_and_justification_are_independent_options() {
    // ^GS p. 217, Table 29 p. 1582: specified baseline is 3/4 of 24 dots.
    // Raw ZD621 controls use native row 23 and ignore FO/FT justification.
    let ft = "^FT100,100^GSN,24,24^FDA";
    assert_eq!(bounds(&raster(ft, SPECIFICATION)), (100, 82, 115, 97));
    assert_eq!(bounds(&raster(ft, ZD621_203_DPI)), (100, 77, 115, 92));
    let mut options = ZD621_203_DPI;
    options.compatibility.graphic_symbol_last_row_baseline = false;
    assert_eq!(bounds(&raster(ft, options)), (100, 82, 115, 97));
    let fo = "^FO100,100,1^GSN,24,24^FDA";
    assert_eq!(bounds(&raster(fo, ZD621_203_DPI)), (100, 100, 115, 115));
    options = ZD621_203_DPI;
    options.compatibility.graphic_symbol_ignores_justification = false;
    assert_eq!(bounds(&raster(fo, options)), (74, 100, 89, 115));
    assert_eq!(raster(ft, options).pixels, raster(ft, ZD621_203_DPI).pixels);
    assert_eq!(raster(fo, SPECIFICATION).pixels, raster(fo, options).pixels);
}

#[test]
fn justified_word_rounding_is_an_independent_printer_option() {
    // ^FB p. 187 distributes slack between words. The 72 raw controls in
    // field-block-rounding-zd621-v1 distinguish ceiling from nearest/floor.
    let body = "^FO50,50^A0N,24,24^FB140,2,4,J^FDAB CD EF AB CD";
    let printer = raster(body, ZD621_203_DPI);
    let mut options = ZD621_203_DPI;
    options.compatibility.block_justification_rounds_up = false;
    let nearest = raster(body, options);
    assert_eq!(nearest.pixels, raster(body, SPECIFICATION).pixels);
    assert_eq!(bounds(&printer), bounds(&nearest));
    assert_eq!(
        printer
            .pixels
            .iter()
            .zip(&nearest.pixels)
            .filter(|(a, b)| a != b)
            .count(),
        110
    );
}

#[test]
fn automatic_hyphen_layout_and_ci27_glyph_are_independent() {
    // ^FB p. 187 requires automatic hyphenation of overlong words. Raw
    // field-block-hyphenation-zd621-v1 controls distinguish soft-hyphen width,
    // strict fits, retained final-chunk space and CI27's incorrect eth glyph.
    let body = "^CI27^FO50,50^AAN,9,5^FB30,6,2,L^FDABCDEFGHIJKLMNOP";
    let printer = raster(body, ZD621_203_DPI);
    let mut corrected = ZD621_203_DPI;
    corrected.compatibility.block_hyphenation_ci27_uses_eth = false;
    let hyphen = raster(body, corrected);
    assert_ne!(printer.pixels, hyphen.pixels);
    assert_eq!(
        hyphen.pixels,
        raster(&body.replace("CI27", "CI28"), ZD621_203_DPI).pixels
    );
    let literal = "^CI27^FO50,50^AAN,9,5^FH^FD_AD_F0";
    assert_eq!(
        raster(literal, corrected).pixels,
        raster(literal, ZD621_203_DPI).pixels
    );

    corrected.compatibility.block_hyphenation_printer_layout = false;
    let generic = raster(body, corrected);
    assert_eq!(generic.pixels, raster(body, SPECIFICATION).pixels);
    let explicit = "^FO50,50^AAN,9,5^FDABCD-^FS^FO50,61^AAN,9,5^FDEFGH-^FS^FO50,72^AAN,9,5^FDIJKL-^FS^FO50,83^AAN,9,5^FDMNOP";
    assert_eq!(generic.pixels, raster(explicit, SPECIFICATION).pixels);
    assert_ne!(generic.pixels, hyphen.pixels);
}

#[test]
fn graphic_box_dimensions_default_to_and_are_at_least_thickness() {
    // Zebra Programming Guide, ^GB, p. 210: default/minimum dimensions
    // are t; its zero-width example is a vertical line. Raw printer controls:
    // box-minimum-zd621-v1 (square and rounded corners).
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for rounding in [0, 1, 8] {
            for (args, normalized) in [
                ("80,50,100", "100,100,100"),
                ("0,50,20", "20,50,20"),
                ("50,0,20", "50,20,20"),
                ("0,0,20", "20,20,20"),
                (",,20", "20,20,20"),
                (",50,20", "20,50,20"),
                ("50,,20", "50,20,20"),
                ("1,1,20", "20,20,20"),
            ] {
                let actual = raster(&format!("^FO40,40^GB{args},B,{rounding}"), options);
                let expected = raster(&format!("^FO40,40^GB{normalized},B,{rounding}"), options);
                assert_eq!(
                    actual.pixels, expected.pixels,
                    "{args}, rounding {rounding}"
                );
            }
        }
    }
}

#[test]
fn graphics_do_not_inherit_field_rotation() {
    // ^FW p. 208 only rotates commands with an orientation parameter.
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for shape in [
            "^GB80,50,3",
            "^GC45,3",
            "^GE80,40,3",
            "^GD80,50,3,B,L",
            "^GFA,4,4,1,AA5555AA",
        ] {
            for origin in ["FO", "FT"] {
                let normal = raster(&format!("^FWN^{origin}100,100{shape}"), options);
                for rotation in ['R', 'I', 'B'] {
                    assert_eq!(
                        normal.pixels,
                        raster(&format!("^FW{rotation}^{origin}100,100{shape}"), options).pixels
                    );
                }
            }
        }
    }
}

#[test]
fn graphic_baseline_and_origin_clamping_are_independent() {
    // Empirical ZD621 controls: graphic-placement-zd621-v1. FT p. 205
    // identifies the bottom of the graphic area as its typesetting origin.
    let body = "^FT40,100^GFA,8,8,1,8000000000000000";
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)), (40, 93, 41, 94));
    let mut options = ZD621_203_DPI;
    options.compatibility.graphic_ft_last_row_baseline = false;
    assert_eq!(bounds(&raster(body, options)), (40, 92, 41, 93));
    let body = "^LS12^FO4,40^GB20,10,10";
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)), (0, 40, 20, 50));
    options = ZD621_203_DPI;
    options.compatibility.graphic_clamps_negative_origin = false;
    assert_eq!(bounds(&raster(body, options)), (0, 40, 12, 50));
    let body = "^FT40,10^GB20,10,10";
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)), (40, 0, 60, 10));
    assert_eq!(bounds(&raster(body, options)), (40, 1, 60, 11));
}

#[test]
fn narrow_field_block_preview_behavior_is_optional() {
    // ^FB p. 186 says text does not print below font width. ZD621 controls
    // in field-block-narrow-zd621-v1 instead force individual characters.
    let body = "^CI27^FO50,50^AAN,9,5^FB1,4,0,L^FDABC";
    assert!(raster(body, SPECIFICATION).pixels.iter().all(|&p| p == 255));
    let mut options = ZD621_203_DPI;
    options.compatibility.block_narrow_printer_layout = false;
    assert!(raster(body, options).pixels.iter().all(|&p| p == 255));
    assert!(raster(body, ZD621_203_DPI).pixels.contains(&0));
    let mut options = SPECIFICATION;
    options.compatibility.block_narrow_printer_layout = true;
    assert_eq!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
    // Negative center/right slack is clamped for a forced glyph.
    for align in ["C", "R", "J"] {
        assert_eq!(
            raster(body, ZD621_203_DPI).pixels,
            raster(&body.replace(",0,L", &format!(",0,{align}")), ZD621_203_DPI).pixels
        );
    }
}

#[test]
fn centered_blocks_quantize_before_rotation_only_when_selected() {
    // Empirical ^FB p. 187 rounding: field-block-centering-zd621-v1.
    // ABC advances 18 dots in resident A, leaving a half-dot center at 19.
    let mut options = ZD621_203_DPI;
    options.compatibility.block_center_rounds_down = false;
    let normal = "^FO80,80^AAN,9,5^FB19,1,0,C^FDABC";
    assert_eq!(
        raster(normal, options).pixels,
        raster(normal, ZD621_203_DPI).pixels
    );
    let inverted = normal.replace("AAN", "AAI");
    assert_ne!(
        raster(&inverted, options).pixels,
        raster(&inverted, ZD621_203_DPI).pixels
    );
    assert_eq!(
        raster(&inverted, ZD621_203_DPI).pixels,
        raster(&inverted.replace(",0,C", ",0,L"), ZD621_203_DPI).pixels
    );
}

#[test]
fn negative_text_origins_are_profile_controlled() {
    // ZD621 text-edge-zd621-v1 captures; ^FO p. 201, ^FT p. 205, ^LS p. 296.
    // Field shift is clamped before glyph placement, preserving all letters.
    let body = "^LS20^FO5,50^AAN,9,5^FDABC";
    let mut options = ZD621_203_DPI;
    options.compatibility.text_clamps_negative_origins = false;
    assert_eq!(bounds(&raster(body, ZD621_203_DPI)), (0, 50, 17, 57));
    assert_eq!(
        raster(body, options).pixels,
        raster(body, SPECIFICATION).pixels
    );
    assert_ne!(
        raster(body, options).pixels,
        raster(body, ZD621_203_DPI).pixels
    );
    // Positive ordinary and block fields must retain their exact pixels.
    for origin in ["FO", "FT"] {
        for rotation in ['N', 'R', 'I', 'B'] {
            for block in ["", "^FB40,4,0,L"] {
                let body = format!("^{origin}180,180^AA{rotation},9,5{block}^FDAB CD");
                assert_eq!(
                    raster(&body, options).pixels,
                    raster(&body, ZD621_203_DPI).pixels
                );
            }
        }
    }
}

#[test]
fn rotated_block_right_origins_are_independent_of_clamping() {
    // ZD621 block-right-origin controls: ^FB p. 187 interacts with ^FO p. 201.
    let mut options = ZD621_203_DPI;
    options.compatibility.text_clamps_negative_origins = false;
    let body = "^FO200,100,1^AAB,9,5^FB40,4,3,L^FDAB CD";
    let printer = raster(body, options);
    options
        .compatibility
        .block_fo_right_justification_printer_layout = false;
    assert_ne!(printer.pixels, raster(body, options).pixels);
    assert_eq!(
        printer.pixels,
        raster(&body.replace("FO200", "FO209"), options).pixels
    );
    let plain = "^FO200,100,1^AAB,9,5^FDAB CD";
    assert_eq!(
        raster(plain, options).pixels,
        raster(plain, ZD621_203_DPI).pixels
    );
}

#[test]
fn field_block_backslash_encoding_departure_is_optional() {
    // ^FB p. 187 Item 1 requires CI13; captured CI27 controls are in
    // field-block-backslash-zd621-v1. Plain fields do not use FB escapes.
    let input = br"^XA^PW400^LL400^CI27^FO50,50^AAN,9,5^FB200,4,0,L^FDABC\\DEF^FS^XZ";
    let mut options = ZD621_203_DPI;
    assert!(render(input, options).is_ok());
    options.compatibility.block_backslash_without_ci13 = false;
    assert!(render(input, options).is_err());
    assert!(render(input, SPECIFICATION).is_err());
    let mut options = SPECIFICATION;
    options.compatibility.block_backslash_without_ci13 = true;
    assert!(render(input, options).is_ok());
    for ci in [0, 28] {
        let source = String::from_utf8(input.to_vec())
            .unwrap()
            .replace("CI27", &format!("CI{ci}"));
        assert!(render(source.as_bytes(), ZD621_203_DPI).is_err());
    }
}
