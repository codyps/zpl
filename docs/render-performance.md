# Rendering performance

## Follow-up (2026-10-02)

The optimized renderer is fastest on all five PNG workloads among the seven
renderers measured on this host. The lead over the next-fastest library is
1.37–6.91×. This is a ranking for these fixtures, versions, and host, not a
claim about every label or platform. Differences in library coverage and output
fidelity remain separate from timing.

### Changes

- Validate each completed field once during scene construction. Dimensions and
  the document segment budget remain checked; output adapters still validate
  the entire scene before touching their destination. Previously each field
  revalidated all earlier draws, making this work quadratic in field count.
- Calculate field anchors and baseline corrections once per field. Check glyph
  bounds for edge clamping instead of transforming every glyph vertex twice.
  Cull invisible text rectangles in place.
- Join identical consecutive bitmap runs only when their edges coincide
  exactly. This reduces allocations, placement work, validation, and raster
  edges while retaining fractional gaps/overlaps and the even-odd ink region.
  Reserve a bounded initial capacity for text row spans.
- Schedule edges at their actual pixel-center crossing rows and reuse sorted
  intersections until vertical edges enter or leave the active set. Keep the
  original conservative scan-budget accounting and general curve converter.
- Rasterize PNG directly into a packed one-bit destination. It uses the same
  scan converter and ordered black/white/invert paints as the public grayscale
  raster. PNG follows [§7.2 scanline packing and §11.2.1 grayscale depths](https://www.w3.org/TR/png-3/#7Scanlines);
  the existing eight-bit `raster_diff::Png::encode_gray` remains available.

No runtime dependencies, scene cache, requested-text cache, CPU-specific
instructions, printer-profile changes, or accuracy-baseline changes were added.
The bitmap-run optimization also reduces SVG path data.

### Fresh cross-library measurements

Linux x86_64, AMD Ryzen 3 PRO 5350GE, Rust/Cargo 1.98.1, release builds.
The adapters and fixtures come from comparison revision
[`e5f2ec13168bad1c8ad41081b3250eb0bc4b8dcb`](https://github.com/codyps/zpl-comparison/tree/e5f2ec13168bad1c8ad41081b3250eb0bc4b8dcb).
Library versions: Labelize 1.6.0, Forge 0.3.2, Rust FFI (`zpl-rs`) 0.1.8,
BinaryKits Viewer 1.3.1; Go source is pinned to
`78b181940c76efa25571c48a2257a130e48a6191`, and ZPLr uses the comparison's npm
lockfile. Go 1.26.8, .NET SDK 8.0.425, Node 24.21.0. Rust library versions are
pinned; this run uses a locally resolved transitive lockfile, not a replay of
the published comparison's dependency graph. The published Forge archive was
checked against the local source used for its build.

Times below are **microseconds per complete parse → scene → PNG operation**,
including destruction and excluding file I/O. All inputs use 400×300 dots and
203 DPI; zpl explicitly selects `ZD621_203_DPI`. Each cell uses five fresh
processes, each warming for at least 250 ms and three operations, then measuring
a calibrated approximately 200 ms batch. The table reports the median batch
average, not per-label tail latency. Jobs run serially in the comparison's
seeded shuffle. No compilation or browser checks ran during this collection;
the host has no frequency lock or CPU affinity, so small differences should
not be generalized.

| Fixture | zpl | Labelize | Forge | Go | Rust FFI | BinaryKits | ZPLr | Lead over next |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| boxes | 16.67 | 115.25 | 366.94 | 758.47 | 779.11 | 4135.93 | 9219.29 | 6.91× |
| text | 40.94 | 146.70 | 388.44 | 815.53 | 815.74 | 4290.28 | 12225.45 | 3.58× |
| code128 | 42.13 | 143.09 | 368.79 | 776.74 | 783.65 | 5208.59 | 8801.82 | 3.40× |
| qr | 80.69 | 260.35 | 371.22 | 946.60 | 932.22 | 5116.63 | 9343.65 | 3.23× |
| dense | 623.35 | 853.38 | 945.83 | 2692.75 | 2708.26 | 5989.04 | 14246.28 | 1.37× |

The zpl batch min–max ranges were 16.60–16.73 µs (boxes), 40.74–41.15 µs
(text), 41.98–42.34 µs (Code 128), 80.47–82.36 µs (QR), and
621.68–628.97 µs (dense).

The harness independently decoded every PNG with Pillow, checked dimensions
and nonblank output, and checked the boxes mask. The five zpl PNGs have zero
changed decoded pixels relative to the existing outputs. Each file is now
15,389 bytes instead of 120,394 bytes (87.2% smaller); encoded PNG hashes change
because the bit depth changes. Font paths also have a different decomposition,
with identical rendered ink.

Cross-library collection remains in the separate `zpl-comparison` repository.
To repeat it, build its adapters with the codyps-zpl path dependency pointing
to this checkout, then use its `benchmarks/run.py` or `build/performance.py`
collection procedure with a new output directory. The default pinned zpl source
must be replaced with this revision before rebuilding. Keep the resolved locks,
source/binary hashes, host details, process samples, and decoded output checks
alongside the report; do not compare timings from different hosts as a speedup.

### Local before/after for this change

The same host ran a preserved release binary from baseline
`6aa0435f057f048f0b238e2f86ea06e8de4d12e0`, followed by the final implementation,
with no builds running. The `render-timing` diagnostic measures each stage and
total separately, using five batches per process. Values are microseconds per
operation. The historical encoder stage takes eight-bit pixels; the new one
takes prepacked one-bit pixels. Raster timings here are for the public eight-bit
destination, while the complete PNG operation uses the packed destination.

| Fixture | Scene before → after | Raster before → after | PNG before → after | Total before → after | Speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| boxes | 1.57 → 1.56 | 6.25 → 4.32 | 189.72 → 12.01 | 240.86 → 16.52 | 14.58× |
| text | 44.79 → 19.65 | 19.57 → 10.29 | 90.01 → 12.11 | 157.96 → 41.59 | 3.80× |
| code128 | 6.89 → 6.11 | 69.39 → 25.59 | 89.78 → 12.03 | 168.84 → 42.01 | 4.02× |
| qr | 61.20 → 57.51 | 16.06 → 26.64 | 90.26 → 12.05 | 170.88 → 82.15 | 2.08× |
| dense | 2207.43 → 635.47 | 257.82 → 177.23 | 99.85 → 12.38 | 2761.43 → 869.70 | 3.18× |

The standalone diagnostic retains a reference scene, raster, and encoded output
while timing. The cross-library adapters retain only the input between calls
and use a fresh process for each sample. Their absolute values, especially for
dense text, are not interchangeable. The QR grayscale-raster stage got slower;
its complete PNG operation still improved by 2.08×. The optimization targets
overall rendering, not a claim that every internal stage improved.

### Validation of the final implementation

- `cargo test --locked --release -p raster-diff -p zpl -p zpl-wasm --lib --tests`:
  515 passed, one manual timing test ignored. This includes `printer_accuracy`,
  `conformance_preview`, all font/printer captures, rotations, edge placement,
  parser and barcode regressions, packed-span compositing, and fractional row
  joins. Existing dimensions, hashes, differences, and error assertions remain.
- `cargo clippy --locked --release -p raster-diff -p zpl --all-targets`: passed,
  with three existing constant-assertion warnings in `tests/profiles.rs`.
- Formatting and diff-whitespace checks passed.
- Rebuilt `wasm32-unknown-unknown` with wasm-bindgen CLI 0.2.128; Node Wasm checks
  passed. Local Chromium rendered the boxes and dense labels, downloaded their
  one-bit PNGs, and matched native pixel hashes exactly. This checks the local
  build; the deployed Pages site was not updated.

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

## Local before/after (2026-09-29)

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

The local diagnostic example accepts single-label files, prints PNG and decoded
pixel hashes, and measures scene creation, grayscale rasterization, PNG-only
encoding from prepacked one-bit pixels, and the entire
render-to-PNG operation. Inputs are read before timing, outputs are consumed
with `black_box` and dropped inside timing, and embedded fonts are warmed.
Explicit `^PW`/`^LL` commands override the 400×300 fallback dimensions. The PNG
stage in the historical September measurements used eight-bit grayscale; the
current full PNG adapter rasterizes directly to one bit. Stage medians are
independent and need not sum to the end-to-end median.

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
