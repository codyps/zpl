use std::{env, fs};
use zpl::{
    output::{Adapter, Png, Svg},
    render, Options,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: zpl-to-svg INPUT.zpl OUTPUT.svg|OUTPUT.png".into());
    }
    let doc = render(&fs::read(&args[1])?, Options::default())?;
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
