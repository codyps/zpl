//! Native Rust oracle: writes the complete response without any Go/FFI runtime.
use std::{env, fs};
fn main() {
    let a: Vec<_> = env::args().collect();
    assert_eq!(
        a.len(),
        7,
        "oracle input output format profile width height"
    );
    let result = zpl_go_prototype::render_packet(
        &fs::read(&a[1]).unwrap(),
        a[3].parse().unwrap(),
        a[4].parse().unwrap(),
        a[5].parse().unwrap(),
        a[6].parse().unwrap(),
    );
    fs::write(&a[2], result).unwrap();
}
