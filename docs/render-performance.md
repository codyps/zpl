# Rendering performance

## Investigation (2026-09-29)

The `zpl-comparison` PNG benchmark includes parsing, scene construction,
rasterization, PNG encoding, and releasing the results. It excludes file I/O.
Its saved 2026-09-20 macOS measurements put our simple labels at 1.27–1.53 ms,
and the 48-field dense label at 7.22 ms. The faster Rust renderers (labelize and
forge) were substantially faster, although several other renderers were slower.
Those results measure different implementations and output fidelity, not just
interchangeable implementations of one pixel contract.

Comparison source inspected:
[`39820755f124116ad127b7a486ffc76ee7c2518f`](https://github.com/codyps/zpl-comparison/tree/39820755f124116ad127b7a486ffc76ee7c2518f),
particularly `benchmarks/adapters/rust/src/main.rs`, `benchmarks/README.md`,
`benchmarks/fixtures`, and `docs/benchmarks/results.json`.

Local measurements isolated two costs:

- PNG encoding was spending about 0.85–1.04 ms on each 400×300 canvas, even
  boxes requiring only about 11 µs to construct and rasterize. The encoder
  writes stored DEFLATE blocks, but calculated CRC one bit at a time and
  reduced both Adler accumulators modulo 65521 for every byte.
- Dense text spent about 3.43 ms constructing the scene, versus 0.35 ms
  rasterizing it. Individual glyph paths needed for printer edge clamping
  went through string layout again: allocating a string, collecting row maps,
  sorting spans, and looking up the same glyph again to measure its advance.
  These duplicate steps contribute to the cost; removing them does not
  eliminate the broader cost of building and positioning text paths.

The checksum implementation now processes CRC eight bytes at a time with
compile-time tables, reduces Adler accumulators in bounded blocks, and reserves
stored-stream capacity up front. The polynomial and checksum semantics remain
those in [PNG section 5.5](https://www.w3.org/TR/PNG/#5CRC-algorithm) and
[RFC 1950 section 2.2](https://www.rfc-editor.org/rfc/rfc1950#section-2.2).
The same Adler helper verifies decoded zlib streams. No runtime dependency or
CPU-specific instruction requirement was added.

Individual glyph paths now emit the already disjoint bitmap spans directly and
reuse the selected glyph's advance. Full strings still merge overlapping ink;
printer profiles, clipping, output-independent scenes, and raster compositing
are unchanged.

## Local before/after

Linux x86_64, AMD Ryzen 3 PRO 5350GE, Rust/Cargo 1.98.1, release profile.
Baseline source: `f54745fcf41ba7a8fda5070e002e114c0561e09c`.
Both runs used the same five comparison fixtures, explicit ZD621 profile,
400×300 canvas, and 203 DPI. Values are microseconds per operation.

| Fixture | Scene before → after | Raster before → after | PNG before → after | Total before → after | Total speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| boxes | 2.23 → 2.44 | 8.57 → 7.85 | 1043.79 → 263.11 | 993.38 → 299.61 | 3.32× |
| text | 79.90 → 63.73 | 22.65 → 22.33 | 879.00 → 111.03 | 1155.70 → 214.78 | 5.38× |
| code128 | 9.29 → 9.22 | 81.37 → 88.44 | 970.61 → 127.82 | 1124.62 → 233.98 | 4.81× |
| qr | 76.42 → 69.32 | 20.89 → 19.32 | 861.77 → 115.34 | 968.68 → 240.61 | 4.03× |
| dense | 3431.64 → 3084.51 | 350.08 → 360.27 | 851.54 → 125.25 | 4339.93 → 3971.63 | 1.09× |

Each stage and total is measured independently: 250 ms warm-up, calibration to
approximately 200 ms batches, median of five batches in one process. Their
medians need not add up. This shared host has no CPU affinity or frequency lock;
the spread between workloads, especially boxes PNG encoding, cautions against
small-percentage conclusions. No builds were running during these timings.
These are local before/after measurements, **not** a fresh cross-library ranking
or an update to the comparison site's saved macOS results.

All five encoded PNGs remained byte-identical (120394 bytes each):

| Fixture | SHA-256 before and after |
| --- | --- |
| boxes | `684f04c5151af510d82f2b0ca9dd925db242eaa6f8332ecd2d284cd1c57531da` |
| text | `9820ea9c166fd553662fb79d6bea95dca346981bb649a672f334dcfbd62677b7` |
| code128 | `0133efd20b138ddde0fc823c9830b014d314b7642fe97e174d48a8c6b95b47e1` |
| qr | `2d0e80e1e19cfd467f22cf067688b7e411bd24a17596452794982b8c02b1964a` |
| dense | `c87f782f1960a557f5fb4c511f03411933c9bbe65bbfa9365c99b5dd7177d3c4` |

## Reproduce and validate

The local diagnostic example accepts single-label files, prints a PNG hash,
and measures scene creation, rasterization, PNG-only encoding, and the entire
render-to-PNG operation. Inputs are read before timing, outputs are consumed
with `black_box` and dropped inside timing, and embedded fonts are warmed.
Explicit `^PW`/`^LL` commands override the 400×300 fallback dimensions.

```sh
cargo run --locked --release -p zpl --example render-timing -- \
  ../zpl-comparison/benchmarks/fixtures/{boxes,text,code128,qr,dense}.zpl
cargo test --locked --release -p raster-diff -p zpl --lib
cargo test --locked --release -p zpl --tests
cargo fmt --all -- --check
```

Checksum tests compare against the bitwise/per-byte definitions at CRC tail,
Adler reduction, and DEFLATE block boundaries, including all-255 overflow
stress. Glyph tests compare exact paths and errors with full glyph layout
across resident faces, fractional scaling, controls, and fallback characters.
The integration suite includes `printer_accuracy`, `conformance_preview`,
font captures, text-edge placement, rotations, and raster output contracts.
No capture or accuracy baseline needs updating.
