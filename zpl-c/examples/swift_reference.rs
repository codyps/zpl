//! Direct Rust parity fixtures for the Swift binding, independent of the C ABI.
use std::{env, fs, path::PathBuf};
use zpl::{
    output::{raster::rasterize, Adapter, Pdf, Png, Svg},
    render::profiles,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env::args().nth(1).expect("fixture output directory"));
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("version.txt"), env!("CARGO_PKG_VERSION"))?;
    let source = b"^XA^PW64^LL48^FO4,4^GB12,8,2^FS^FO24,4^A0N,18,12^FDZ^FS^XZ^XA^PW64^LL48^FO8,8^GB20,12,3^FS^XZ";
    fs::write(directory.join("source.zpl"), source)?;
    for (name, profile) in [
        ("zd621", profiles::ZD621_203_DPI),
        ("specification", profiles::SPECIFICATION),
        ("zq610Plus", profiles::ZQ610_PLUS_203_DPI),
        ("gx430t", profiles::GX430T_300_DPI),
    ] {
        let document = zpl::render(source, profile)?;
        fs::write(
            directory.join(format!("{name}-warnings.txt")),
            document.warnings.join("\n"),
        )?;
        fs::write(
            directory.join(format!("{name}-pages.pdf")),
            Pdf.encode_pages(&document.labels)?,
        )?;
        for (index, scene) in document.labels.iter().enumerate() {
            let prefix = directory.join(format!("{name}-{index}"));
            fs::write(prefix.with_extension("png"), Png.encode(scene)?)?;
            fs::write(prefix.with_extension("svg"), Svg.encode(scene)?)?;
            fs::write(prefix.with_extension("pdf"), Pdf.encode(scene)?)?;
            fs::write(prefix.with_extension("gray"), rasterize(scene)?.pixels)?;
            fs::write(
                prefix.with_extension("info"),
                format!("{},{},{}", scene.width, scene.height, scene.dpi),
            )?;
        }
    }
    fs::write(
        directory.join("probe.ttf"),
        include_bytes!(
            "../../zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf"
        ),
    )?;
    Ok(())
}
