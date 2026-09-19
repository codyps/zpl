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
