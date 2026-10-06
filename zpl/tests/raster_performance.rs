//! Opt-in stage timings for representative native previews. Never updates baselines.
use std::{
    fs,
    hint::black_box,
    path::Path,
    time::{Duration, Instant},
};
#[path = "support/digest.rs"]
mod digest;

#[test]
#[ignore = "manual stage timing; run with --ignored --nocapture"]
fn representative_raster_stages() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    println!(
        "x86 SHA instructions available: {}",
        std::arch::is_x86_feature_detected!("sha")
    );
    #[cfg(target_arch = "aarch64")]
    println!(
        "ARM SHA-2 instructions available: {}",
        std::arch::is_aarch64_feature_detected!("sha2")
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cases = [
        "barcode-aligned-zd621-v1/symbol-qr",
        "barcode-aligned-zd621-v1/symbol-databar_upce",
        "maxicode-zd621-v1/5-baseline",
        "shipping-fonts-zd621-v1/original",
        "empty-qr-zd621-v1/after-code",
    ];
    let mut elapsed = [Duration::ZERO; 4];
    let mut pixels = 0;
    for _ in 0..5 {
        for name in cases {
            let input = fs::read(root.join(format!("{name}.zpl"))).unwrap();
            let png = fs::read(root.join(format!("{name}.png"))).unwrap();
            let start = Instant::now();
            let reference = raster_diff::Raster::decode_png(&png).unwrap();
            elapsed[0] += start.elapsed();
            let start = Instant::now();
            let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI);
            let Ok(doc) = doc else {
                assert!(
                    name.ends_with("symbol-databar_upce"),
                    "unexpected render error: {name}"
                );
                continue;
            };
            let candidate = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
            elapsed[1] += start.elapsed();
            let start = Instant::now();
            black_box(raster_diff::compare_stats(&reference, &candidate, false).unwrap());
            elapsed[2] += start.elapsed();
            let start = Instant::now();
            black_box(digest::sha256(black_box(&candidate.pixels)));
            elapsed[3] += start.elapsed();
            pixels += candidate.pixels.len();
        }
    }
    println!(
        "{pixels} pixels; decode={:?} render={:?} compare={:?} sha256={:?}",
        elapsed[0], elapsed[1], elapsed[2], elapsed[3]
    );
}
