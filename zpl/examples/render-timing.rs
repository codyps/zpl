//! Opt-in release-mode stage timings for local, single-label ZPL files.
//! See docs/render-performance.md for the measurement boundary.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    hint::black_box,
    time::{Duration, Instant},
};
use zpl::output::{raster::rasterize, Adapter, Png};

fn measure(mut operation: impl FnMut()) -> f64 {
    let warm = Instant::now();
    let mut iterations = 0;
    while warm.elapsed() < Duration::from_millis(250) || iterations < 3 {
        operation();
        iterations += 1;
    }
    let iterations = ((iterations as f64 * 0.2 / warm.elapsed().as_secs_f64()) as usize).max(1);
    let mut samples = [0.; 5];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        *sample = start.elapsed().as_secs_f64() * 1e6 / iterations as f64;
    }
    samples.sort_by(f64::total_cmp);
    samples[2]
}

fn main() {
    assert!(
        env::args_os().len() > 1,
        "usage: render-timing FILE.zpl ..."
    );
    for file in env::args_os().skip(1) {
        let input = fs::read(&file).unwrap();
        let options = zpl::Options {
            width: 400,
            height: 300,
            dpi: 203,
            ..zpl::render::profiles::ZD621_203_DPI
        };
        let doc = zpl::render(&input, options).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let scene = &doc.labels[0];
        let raster = rasterize(scene).unwrap();
        let png = Png.encode(scene).unwrap();
        println!(
            "{}: {}x{}, PNG {} bytes, sha256={}",
            file.to_string_lossy(),
            scene.width,
            scene.height,
            png.len(),
            Sha256::digest(&png)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let render = measure(|| {
            black_box(zpl::render(black_box(&input), options).unwrap());
        });
        let rasterize = measure(|| {
            black_box(rasterize(black_box(scene)).unwrap());
        });
        let encode = measure(|| {
            black_box(raster_diff::Png::encode_gray(black_box(&raster), scene.dpi).unwrap());
        });
        let total = measure(|| {
            let doc = zpl::render(black_box(&input), options).unwrap();
            black_box(Png.encode(&doc.labels[0]).unwrap());
        });
        println!("median us/op: scene={render:.2} raster={rasterize:.2} png={encode:.2} total={total:.2}");
    }
}
