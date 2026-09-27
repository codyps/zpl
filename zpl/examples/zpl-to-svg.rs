use std::{env, fs};
use zpl::{
    output::{Adapter, Png, Svg},
    render,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err(
            "usage: zpl-to-svg INPUT.zpl OUTPUT.svg|OUTPUT.png [--explicit-qr-mask] [--profile zd621|zd621-preview|zq610-plus|specification]".into(),
        );
    }
    let mut options = zpl::render::profiles::ZD621_203_DPI;
    let mut explicit_mask = false;
    let mut extra = args[3..].iter();
    while let Some(arg) = extra.next() {
        match arg.as_str() {
            "--explicit-qr-mask" => explicit_mask = true,
            "--profile" => {
                options = match extra.next().map(String::as_str) {
                    Some("zd621") => zpl::render::profiles::ZD621_203_DPI,
                    Some("zd621-preview") => {
                        let mut preview = zpl::render::profiles::ZD621_203_DPI;
                        preview.compatibility.preview_width_quantum = Some(64);
                        preview.compatibility.preview_width_latched_at_first_draw = true;
                        preview
                    }
                    Some("zq610-plus") => zpl::render::profiles::ZQ610_PLUS_203_DPI,
                    Some("specification") => zpl::render::profiles::SPECIFICATION,
                    _ => return Err("unknown or missing printer profile".into()),
                };
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    if explicit_mask {
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
