# Automatic glyph and encoding recovery

`zpl-font-extract recover` is the single automatic pipeline. It discovers fonts,
recovers glyphs, surveys encoding mappings, verifies captures, writes portable
`fonts.json`, and generates `rust/{fonts.rs,bitmaps.bin,catalog.json}`.

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _font-cache --out _recovered-fonts
```

Without `--source` or `--font`, it lists named `.FNT` selectors on Z/E/R through
read-only SGD `file.dir` requests (default port 9100), and includes resident A–H
and the separate `^GS` face (`@`). Directory discovery reads names only. No font
objects are downloaded or decoded. A listed name does not guarantee a usable
font: blank previews and filename fallback remain explicit mapping outcomes.

Use `--font A,Z:A.FNT` for a subset. To start with known glyphs or resume a JSON
collection, add `--source fonts.json`; omitted selectors then mean all fonts in
that input. Both cases run the same recovery and compilation sequence. Verified
legacy v1 JSON is migrated on input; new output always uses the current format.

The former `collection survey` workflow and the old byte-only compiler are
removed. Use `recover --source ...` and `compile` instead. The bundled renderer
reader is retained because the existing `zd621` dataset still uses it.

## Capture, cache and replay

Credentials come from `ZPL_USERNAME` and `ZPL_PASSWORD`. Captures use the HTTP
Preview Label form, registration markers and native canvases; they do not submit
physical print jobs. Avoid concurrent preview clients. The CLI locks its cache
and printer origin, rejects redirects/cross-origin image URLs, bounds responses,
and spaces captures five seconds apart by default. Failures stop the run without
an implicit reset or automatic retry.

The read-only JSON settings channel (default port 9200) provides printer serial
and software version before and after recovery. Captures are scoped to this
identity and source-glyph hashes. Directory selectors and identity are cached
alongside the plans, ZPL, PNGs, repeats and hash receipts. Use `--sgd-port` and
`--identity-port` to override the metadata ports.

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _font-cache \
  --out _replayed-fonts --offline
```

Replay needs the same inputs and options. It makes no network requests and fails
on absent or corrupt evidence. An interrupted run reuses completed captures;
existing output directories are never overwritten. Use a new cache after other
printer-state changes.

`plan --font A --out _initial-plan` writes initial resident calibration requests
without contacting the printer. Subsequent pages are adaptive and remain in the
capture cache.

## Measurement and glyph discovery

For resident fonts without an input collection, calibration compares `^FO` and
`^FT`, measures advances using sentinel compositions, and fits native cell sizes
from integer magnification transitions. Independent baseline/scale controls and
fresh mixed-text pages verify the recovered pixels and metrics. These are stages
of `recover`, not a separate export pipeline.

Every encoding pass captures isolated, doubled and tripled inputs. It first
matches supplied or previously recovered glyphs, preserving all indistinguishable
candidates. A new visible glyph is recovered from its isolated pixels and the
advance inferred from the pair; both pair and triple must recompose exactly.
The third sample verifies a composition not used to infer the advance. Newly
recovered glyphs are immediately available to subsequent inputs and encodings.
Named fonts can therefore be recovered without a pre-existing glyph collection.

All pages check dimensions, clipping, registration and stray ink, and each pass
requires an exact independent repeat of its first page. Named-font controls vary
the default face to detect filename fallback. A blank input alone cannot establish
a glyph or its advance, and remains unresolved. Non-composable observations are
also explicit; they are never silently promoted to glyphs.

`--bound` controls resident native-size calibration. `--width` and `--height`
select the preview canvas (832×4096 by default). Named-font discovery captures the
size-one preview strike; calibrated cell metrics are optional when unavailable.
Out-of-bounds or unstable previews fail rather than publishing clipped glyphs.

## Encodings and coverage

Defaults cover all 256 inputs for CI0 source remapping and CI0–13/27/31/33–36,
plus a UTF-8 discovery repertoire: U+0000–052F, U+2000–26FF, U+F000–F0FF and
U+FFFD–FFFF. `--encoding` accepts a comma-separated subset, using `source` for
CI0 source positions. `--codes` supplies explicit decimal inputs. Four control
inputs (48, 49, 65, 66) are always included in mapping passes.

```sh
cargo run --locked -p zpl-font-extract -- recover \
  --host http://printer.local/ --cache _font-cache --out _focused \
  --font Z:A.FNT --encoding 28 --codes 32,65,66,233,923
```

Input bytes, CI0 source positions, Unicode scalars and opaque glyph IDs are
separate namespaces. UTF-8 input is never truncated to a byte. Actual field bytes
are escaped through `^FH`. UTF-16 HTTP previews remain disabled after the tested
ZD621 stopped responding to them.

This automates the supported survey; it cannot prove recovery of glyphs no tested
input can select, or claim all Unicode/multibyte encodings were swept. The JSON
records the requested scope, exact tested keys, unresolved/ambiguous outcomes,
and coverage of supplied glyphs. An absent map key is untested. Imported
unverified candidates have a separate namespace/status and do not reduce measured
coverage gaps.

## JSON and compilation

Current output is `zebra-bitmap-collection` version 3. Each font contains a name,
source hash, optional measured cell width/height/baseline/space advance, glyph
records, and per-encoding maps. Records retain 16-bit IDs, signed bearings,
advance, declared dimensions and byte-padded bitmap rows. Original padding ink
in imported glyphs is retained. There are no binary headers, slot tables or
format flags. The earlier draft v2 schema is rejected.

The content SHA-256 covers compact JSON serialization of `(schema, version,
fonts, provenance, coverage, candidate_code_pages)`. Struct field order follows
the Rust types; arbitrary provenance object keys are sorted. `Collection::new`
and `seal` derive coverage/hash; `load` and `save` validate them. Documents have
a 256 MiB read/write limit. Hashes detect edits; they are not printer signatures.

```sh
cargo run --locked -p zpl-font-extract -- compile _recovered-fonts/fonts.json \
  --out _compact-fonts
cargo run --locked -p zpl-font-extract -- merge-evidence fonts.json \
  --report observations.json --evidence-root _captures --out _merged.json
```

`compile` accepts current JSON and verified legacy v1 input and emits one table
format using `zpl_bitmap_fonts::collection`. `merge-evidence` accepts verified v1
data, hash-checked `zebra-font-encoding-observations-v1` reports, and separately
labelled `zebra-font-mapping-candidates-v1` reports. Contradictory observations
fail. Candidate reports contain `code_pages` (ID, 256 character keys, provenance)
and `font_candidates` (source hash, record IDs/character keys, provenance).

The generated reader uses static arrays without allocation, runtime JSON or new
runtime dependencies. It shares glyph records, bitmap payloads and mapping
candidate sets. Keep generated `fonts.rs` beside `bitmaps.bin`.

```rust,ignore
let font = generated::font_by_name("Z:A.FNT").unwrap();
let map = font.encoding(zpl_bitmap_fonts::collection::Encoding::Input { ci: 28 }).unwrap();
if let Some(observation) = map.lookup(0x039b) {
    // Inspect status and all candidates; do not silently choose an ambiguous ID.
    for id in observation.candidates {
        let glyph = font.record(*id).unwrap();
        println!("{} {}", glyph.advance, glyph.pixel(0, 0));
    }
}
```

The bundled `zd621` tables and their existing reader remain unchanged. Newly
compiled tables use the unified reader; they are not drop-in replacements for
the old generated module API.

## Validation

```sh
cargo test --locked -p zpl-font-extract -p zpl-bitmap-fonts --all-features
cargo clippy --locked -p zpl-font-extract -p zpl-bitmap-fonts --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Tests include independent simulated printer responses, loopback HTTP/cache replay,
Unicode glyph discovery without seed data, multiple encodings in one recovery,
calibration holdouts, ambiguous/blank inputs, bounded identity/directory queries,
legacy migration, JSON tampering and compiled-reader execution.
