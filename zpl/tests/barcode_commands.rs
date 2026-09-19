//! ^CV and ^FM: Zebra ZPL II Programming Guide pp. 167 and 197–199.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use zpl::{
    output::raster::rasterize,
    render,
    render::profiles::{SPECIFICATION, ZD621_203_DPI},
    Options,
};

fn pixels(input: &str, options: Options) -> Vec<u8> {
    rasterize(&render(input.as_bytes(), options).unwrap().labels[0])
        .unwrap()
        .pixels
}

#[test]
fn validation_state_and_failures() {
    for (cmd, data) in [
        ("B8N,60,N,N", "ABC"),
        ("B8N,60,N,N", "12345671"),
        ("B8N,60,N,N", "1"),
        ("B8N,60,N,N", "1234567890"),
        ("BXN,4,0,10", "ABC"),
        ("BCN,60,Z,N", "ABC"),
    ] {
        let field = format!("^FO40,40^{cmd}^FD{data}^FS");
        assert!(render(format!("^XA{field}^XZ").as_bytes(), SPECIFICATION).is_err());
        let doc = render(
            format!("^XA^CVY{field}^XZ^XA{field}^XZ").as_bytes(),
            SPECIFICATION,
        )
        .unwrap();
        assert_eq!(doc.labels.len(), 2);
        let a = rasterize(&doc.labels[0]).unwrap();
        let b = rasterize(&doc.labels[1]).unwrap();
        assert_eq!(a.pixels, b.pixels);
        assert!(a.pixels.contains(&0));
        assert!(render(
            format!("^XA^CVY{field}^CVN{field}^XZ").as_bytes(),
            SPECIFICATION
        )
        .is_err());
    }
    // Unsupported renderer semantics must never be disguised as invalid data.
    assert!(render(b"^XA^CVY^BQN,3^FDLA,ABC^FS^XZ", SPECIFICATION).is_err());
}

#[test]
fn validation_profile_codes_are_independent() {
    for (cmd, data, retail) in [
        ("B8N,60,N,N", "1234567890", true),
        ("BXN,4,140,9", "ABC", false),
    ] {
        let input = format!("^XA^PW832^LL400^CVY^FO60,60^{cmd}^FD{data}^FS^XZ");
        let spec = pixels(&input, SPECIFICATION);
        let printer = pixels(&input, ZD621_203_DPI);
        assert_ne!(spec, printer);
        let mut options = ZD621_203_DPI;
        if retail {
            options.compatibility.validation_retail_long_is_short = false;
        } else {
            options.compatibility.validation_legacy_small_is_parameter = false;
        }
        assert_eq!(spec, pixels(&input, options));
    }
}

#[test]
fn multiple_origins_exclusion_capacity_and_scope() {
    let payload = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(3);
    for command in ["B7N,3,0,3,10,N", "BFN,3,10"] {
        let label =
            |origins: &str| format!("^XA^PW832^LL400^BY1^FM{origins}^{command}^FD{payload}^FS^XZ");
        let full = pixels(&label("40,80,280,80,520,80"), ZD621_203_DPI);
        let exclude = pixels(&label("40,80,e,e,520,80"), ZD621_203_DPI);
        for y in 0..400 {
            for x in 0..832 {
                let i = y * 832 + x;
                if (280..500).contains(&x) {
                    assert_eq!(exclude[i], 255);
                } else {
                    assert_eq!(exclude[i], full[i]);
                }
            }
        }
        assert!(pixels(&label("40,80"), ZD621_203_DPI)
            .iter()
            .all(|&v| v == 255));
        assert_eq!(
            full,
            pixels(&label("40,80,280,80,520,80,700,300"), ZD621_203_DPI)
        );
    }
    let text = "^XA^PW832^LL400^FO40,40^A0N,32,32^FDText^FS^XZ";
    assert_eq!(
        pixels(text, SPECIFICATION),
        pixels(&text.replace("^A0", "^FM100,100,200,200^A0"), SPECIFICATION)
    );
    let baseline = text.replace("^FO40,40", "^FT40,100");
    assert_eq!(
        pixels(&baseline, SPECIFICATION),
        pixels(
            &baseline.replace("^A0", "^FM100,100,200,200^A0"),
            SPECIFICATION
        )
    );
    for bad in ["1", "1,", "e", "-1,0", "0,0.5", "0,32001"] {
        assert!(render(format!("^XA^FM{bad}^XZ").as_bytes(), SPECIFICATION).is_err());
    }
}

#[test]
fn macro_pdf417_parts_decode_and_reassemble() {
    // Each part must be independently decodable, with its own compaction state.
    for payload in [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(3),
        "abcdefghij".repeat(7),
        "1234567890".repeat(12),
    ] {
        for options in [SPECIFICATION, ZD621_203_DPI] {
            let label = format!(
                "^XA^PW1600^LL300^BY3^FM50,50,550,50,1050,50^B7N,6,0,3,10,N^FD{payload}^FS^XZ"
            );
            let doc = render(label.as_bytes(), options).unwrap();
            let raster = rasterize(&doc.labels[0]).unwrap();
            let mut decoded = String::new();
            for x0 in [30u32, 530, 1030] {
                let mut crop = Vec::new();
                for y in 30..130 {
                    crop.extend_from_slice(
                        &raster.pixels[(y * 1600 + x0) as usize..(y * 1600 + x0 + 400) as usize],
                    );
                }
                let found = rxing::helpers::detect_in_luma(
                    crop,
                    400,
                    100,
                    Some(rxing::BarcodeFormat::PDF_417),
                )
                .unwrap();
                decoded.push_str(found.getText());
            }
            assert_eq!(decoded, payload);
        }
    }
}

#[test]
fn macro_profile_overrides_can_be_disabled_separately() {
    let input = "^XA^PW832^LL400^BY1^FM40,80^BFI,2,10^FDABCDEFGHIJ^FS^XZ";
    let reference = pixels(input, ZD621_203_DPI);
    for which in 0..2 {
        let mut options = ZD621_203_DPI;
        match which {
            0 => options.compatibility.macro_pdf417_file_id = None,
            _ => {
                options
                    .compatibility
                    .macro_micropdf417_reverse_origin_omits_side_raps = false
            }
        }
        assert!(pixels(input, options) != reference, "override {which}");
    }
    let mut bad = SPECIFICATION;
    bad.compatibility.macro_pdf417_file_id = Some([900, 0, 0]);
    assert!(render(input.as_bytes(), bad).is_err());
}
