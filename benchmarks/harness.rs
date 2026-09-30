//! Identical external harness for both revisions; see benchmarks/README.md.
//! black_box semantics: https://doc.rust-lang.org/std/hint/fn.black_box.html
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const CASES: &[(&str, &[u8])] = &[
    ("text", b"^XA^PW812^LL600^FO30,30^A0N,40,40^FDShipping label 12345^FS^FO30,100^A0N,28,28^FD123 Example Street^FS^FO30,150^A0N,28,28^FDSpringfield XY 12345^FS^XZ"),
    ("barcodes", b"^XA^PW812^LL600^FO30,30^BY2^BCN,100,Y,N,N^FD123456789012^FS^FO30,200^BQN,2,5^FDLA,https://example.com/track/12345^FS^XZ"),
    ("graphics", b"^XA^PW812^LL600^FO20,20^GB700,500,4^FS^FO50,50^GC200,8^FS^FO300,50^GE350,200,5^FS^FO50,300^GB600,150,80^FS^FO80,330^GB500,80,20,W^FS^XZ"),
];

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let (case, stage) = args[1].split_once('/').expect("case/stage");
    let input = CASES
        .iter()
        .find(|(name, _)| *name == case)
        .expect("known case")
        .1;
    let duration = Duration::from_millis(args[2].parse().expect("milliseconds"));
    assert!(!duration.is_zero());
    let options = zpl::render::profiles::ZD621_203_DPI;
    let scene = zpl::render(input, options).expect("valid benchmark label");
    assert_eq!(scene.labels.len(), 1);
    let image = zpl::output::raster::rasterize(&scene.labels[0]).unwrap();
    assert_eq!((image.width, image.height), (812, 600));
    assert!(image.pixels.contains(&0));
    let mut iterations = 0u64;
    let start = Instant::now();
    // Batch clock reads; all allocations and destruction belong to the measured operation.
    loop {
        for _ in 0..8 {
            match stage {
                "scene" => {
                    black_box(zpl::render(black_box(input), options).unwrap());
                }
                "raster" => {
                    black_box(zpl::output::raster::rasterize(black_box(&scene.labels[0])).unwrap());
                }
                "total" => {
                    let doc = zpl::render(black_box(input), options).unwrap();
                    black_box(zpl::output::raster::rasterize(&doc.labels[0]).unwrap());
                }
                _ => panic!("unknown stage"),
            }
        }
        iterations += 8;
        if start.elapsed() >= duration {
            break;
        }
    }
    println!("{} {}", iterations, start.elapsed().as_nanos());
}
