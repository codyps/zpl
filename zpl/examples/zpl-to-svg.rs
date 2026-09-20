use std::{env, fs};
use zpl::{
    output::{Adapter, Png, Svg},
    render,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 && !(args.len() == 4 && args[3] == "--explicit-qr-mask") {
        return Err(
            "usage: zpl-to-svg INPUT.zpl OUTPUT.svg|OUTPUT.png [--explicit-qr-mask]".into(),
        );
    }
    let mut options = zpl::render::profiles::ZD621_203_DPI;
    if args.len() == 4 {
        options.compatibility.qr_printer_mask_selection = false;
    }
    let doc = render(&fs::read(&args[1])?, options)?;
    if doc.labels.len() != 1 {
        return Err(
            "this CLI requires exactly one label; use the library for multiple labels".into(),
        );
    }
    for warning in doc.warnings {
        eprintln!("warning: {warning}");
    }
    let adapter: Box<dyn Adapter> = match std::path::Path::new(&args[2])
        .extension()
        .and_then(|s| s.to_str())
    {
        Some("svg") => Box::new(Svg),
        Some("png") => Box::new(Png),
        _ => return Err("output extension must be svg or png".into()),
    };
    fs::write(&args[2], adapter.encode(&doc.labels[0])?)?;
    Ok(())
}
