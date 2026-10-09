//! Independent adapter oracle for ExUnit's binding parity test.
//! Scene/output contracts: docs/local-renderer.md, docs/printer-accuracy.md.
use std::{fs, path::PathBuf};
use zpl::{
    output::{self, Adapter},
    render::profiles,
};

fn main() {
    let directory = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    fs::create_dir_all(&directory).unwrap();
    let input = b"^XA^FO2,3^GB10,8,2^FS^XZ^XA^FO1,1^GB3,4,1^FS^XZ";
    for (name, profile) in [
        ("specification", profiles::SPECIFICATION),
        ("zd621", profiles::ZD621_203_DPI),
        ("zq610_plus", profiles::ZQ610_PLUS_203_DPI),
    ] {
        let document = zpl::render(
            input,
            zpl::Options {
                width: 32,
                height: 24,
                ..profile
            },
        )
        .unwrap();
        fs::write(
            directory.join(format!("{name}.pdf")),
            output::Pdf.encode_pages(&document.labels).unwrap(),
        )
        .unwrap();
        for (index, scene) in document.labels.iter().enumerate() {
            for (extension, bytes) in [
                ("png", output::Png.encode(scene).unwrap()),
                ("svg", output::Svg.encode(scene).unwrap()),
                ("pdf", output::Pdf.encode(scene).unwrap()),
                ("raw", output::raster::rasterize(scene).unwrap().pixels),
            ] {
                fs::write(directory.join(format!("{name}-{index}.{extension}")), bytes).unwrap();
            }
        }
    }
}
