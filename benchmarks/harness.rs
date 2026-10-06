//! Identical external harness for both revisions; see benchmarks/README.md.
//! black_box semantics: https://doc.rust-lang.org/std/hint/fn.black_box.html
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

// A–H (including the C/D alias) and GS exercise resident bitmap lookup,
// packed glyph scans, row joining, and integer scaling. Each row has native
// dimensions on the left and 2x dimensions on the right. Font 0 stays in the
// separate text case. Matrix sizes: ZPL Programming Guide Tables 29/31;
// see zpl/tests/fixtures/resident-bc-zd621-v1 and graphic-symbols-zd621-v1.
const BITMAP_TEXT: &[u8] = concat!(
    "^XA^PW812^LL600^CI27",
    "^FO30,20^AAN,9,5^FDAZ09 /W^FS^FO430,20^AAN,18,10^FDW09^FS",
    "^FO30,60^ABN,11,7^FDAZ09 /W^FS^FO430,60^ABN,22,14^FDW09^FS",
    "^FO30,105^ACN,18,10^FDAZ09 /W^FS^FO430,105^ACN,36,20^FDW09^FS",
    "^FO30,155^ADN,18,10^FDAZ09 /W^FS^FO430,155^ADN,36,20^FDW09^FS",
    "^FO30,235^AEN,28,15^FDAZ09 /W^FS^FO430,235^AEN,56,30^FDW09^FS",
    "^FO30,300^AFN,26,13^FDAZ09 /W^FS^FO430,300^AFN,52,26^FDW09^FS",
    "^FO30,370^AGN,60,40^FDAZ09 /W^FS^FO430,370^AGN,120,80^FDW09^FS",
    "^FO30,500^AHN,21,13^FDAZ09 /W^FS^FO430,500^AHN,42,26^FDW09^FS",
    "^FO30,550^GSN,24,24^FDABCDE^FS^FO430,550^GSN,48,48^FDABC^FS",
    "^XZ",
)
.as_bytes();

const CASES: &[(&str, &[u8])] = &[
    ("text", b"^XA^PW812^LL600^FO30,30^A0N,40,40^FDShipping label 12345^FS^FO30,100^A0N,28,28^FD123 Example Street^FS^FO30,150^A0N,28,28^FDSpringfield XY 12345^FS^XZ"),
    ("bitmap-text", BITMAP_TEXT),
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
    if case == "bitmap-text" {
        // Outside timing: require ink in every face/size region so a missing
        // field cannot silently turn this into a cheaper benchmark.
        for (top, bottom) in [
            (20, 60),
            (60, 105),
            (105, 155),
            (155, 235),
            (235, 300),
            (300, 370),
            (370, 500),
            (500, 550),
            (550, 600),
        ] {
            for (left, right) in [(30, 420), (430, 812)] {
                assert!(
                    (top..bottom)
                        .any(|y| image.pixels[y * 812 + left..y * 812 + right].contains(&0)),
                    "blank bitmap face/size region at {left},{top}"
                );
            }
        }
    }
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
