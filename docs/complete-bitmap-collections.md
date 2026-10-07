# Complete bitmap records and encoding mappings

`zpl-font-extract collection` surveys input-to-glyph mappings and compiles
format-independent glyph JSON for `zpl-bitmap-fonts`. Glyph IDs are opaque
16-bit keys, separate from byte inputs and Unicode code points. An observation
can match several indistinguishable glyphs.

The existing `plan`, `recover`, and `compile` commands remain the preview-only
workflow: calibrate, measure glyph pixels and metrics, verify with independent
holdouts, and compile the verified v1 JSON. The collection workflow adds multiple
encodings, Unicode inputs, imported glyph collections, and unresolved mappings.
It does not claim that previews can select every glyph on a printer. UTF-16 HTTP
previews remain disabled because they caused the tested ZD621 to stop responding.

## Portable collection input

Start with `fonts.json` in the `zebra-bitmap-collection` version 3 format.
Each font contains a name, source SHA-256, glyph records, and encoding maps.
Each record contains `id`, `advance`, `left`, `top`, `width`, `height`, and
`bitmap_hex` (byte-padded rows). The top bearing is relative to the `^FT`
baseline. Source hashes scope cached observations to the supplied glyph data.
Blank glyphs are valid records; an absent ID simply has no supplied record.

The format contains no binary font headers, slot tables or format flags.
The earlier draft v2 format is rejected: exporters must supply the portable v3
fields and recompute coverage and the content hash. There is no binary font
file importer in this tool.

The top-level fields are `schema`, `version`, `fonts`, `provenance`,
`content_sha256`, `coverage`, and `candidate_code_pages`. The SHA-256 covers the
compact JSON serialization of the tuple `(schema, version, fonts, provenance,
coverage, candidate_code_pages)`. Struct field order follows the Rust collection
types; keys inside arbitrary JSON provenance objects are sorted. `Collection::new`
and `seal` derive coverage and the hash; `load` and `save` validate both. External
exporters must use the same serialization contract.

## Obtain input mappings

```sh
cargo run --locked -p zpl-font-extract -- collection survey \
  fonts.json --host http://printer.example/ \
  --cache _font-cache --out _font-mapped.json
```

By default the survey visits all collected bitmap fonts, all 256 inputs for CI0
source remapping and CI0–13/27/31/33–36, and a bounded UTF-8 discovery repertoire:
U+0000–052F, U+2000–26FF, U+F000–F0FF and U+FFFD–FFFF. This is not an exhaustive
Unicode or multibyte-code-page sweep. `--font`, `--encoding`, and `--codes` select
focused work; all keys are decimal. For example:

```sh
cargo run --locked -p zpl-font-extract -- collection survey \
  fonts.json --host http://printer.example/ \
  --cache _font-cache --out _font-pilot.json \
  --font Z:A.FNT --encoding 28 --codes 32,65,66,233,923
```

The byte input domain and Unicode domain are explicit in each map. `source`
means CI0 source positions routed through safe field bytes. Non-ASCII Unicode
is UTF-8 encoded before `^FH`; it is never truncated to a byte. Actual field
bytes are always escaped.

Each input is rendered alone, doubled and tripled. Exact ink, bearings and
advance must agree with archived records. Named-font controls vary the default
font to detect filename fallback. Blank input remains unresolved, even if the
archive contains blank records; an ignored input cannot prove which blank
record was selected. Every batch validates page registration, exact native
canvas dimensions and a separately captured repeat page. Duplicates remain a
set of candidate IDs; even a singleton is observed equivalence, not direct
inspection of the printer's internal glyph selection.

Raw PNG/ZPL evidence is cached with integrity receipts and printer locking.
Collection surveys query serial and firmware through the read-only JSON settings
channel (default TCP 9200, configurable with `--identity-port`) before and after
capture. Cache keys include this identity and the source-font hash. Replaying
with `--offline` uses saved identity and evidence without network access;
missing or corrupt cache entries fail. Use a new cache after other printer-state
changes. Errors stop the Rust survey; it does not reset the printer implicitly.
An interrupted run can be resumed from completed, checked cache entries.

## Import existing evidence

```sh
cargo run --locked -p zpl-font-extract -- collection merge-evidence \
  fonts.json \
  --report zpl-bitmap-fonts/data/zd621/fonts.json \
  --report PATH_TO_ENCODING_OBSERVATIONS.json \
  --report PATH_TO_MAPPING_CANDIDATES.json \
  --evidence-root PATH_TO_CAPTURE_DIRECTORY \
  --out _font-mapped.json
```

Accepted evidence formats are the existing verified `zebra-bitmap-fonts` v1
collection, `zebra-font-encoding-observations-v1`, and
`zebra-font-mapping-candidates-v1`. The v1 import checks its verification/content
hash and compares actual glyph pixels and advance against stored records. The
observation import checks reference-font hashes and the saved plan, PNG/ZPL,
repeat and completion receipts. Conflicting observations fail rather than
silently replacing earlier results. Experimental out-of-range source mappings
are not accepted.

Unverified candidate mappings have their own namespace and status. Their raw
code-page tables retain every byte and the original character keys,
including vendor-specific keys. The compact reader only composes these tables
with unverified candidate maps through an explicitly named `candidate()` method.
They never become measured CI mappings automatically.

The candidate report uses schema `zebra-font-mapping-candidates-v1`.
`code_pages` contains objects with `id`, a 256-element `byte_to_character`
array, and optional `provenance`. `font_candidates` is keyed by stored font
name; each value contains `reference_sha256`, optional `provenance`, and
`entries` with `raw_record_id` and `character_codes`. Imports retain the report
hash and supplied provenance. These character keys are not assumed to be Unicode.

## Compile for zpl-bitmap-fonts

```sh
cargo run --locked -p zpl-font-extract -- collection compile \
  _font-mapped.json --out _compiled-fonts
```

The compiler validates schema, content hash, unique ordered glyph IDs, record
bounds, candidate references, mapping domains and evidence categories before
writing anything. It emits `fonts.rs`, `bitmaps.bin`, and `catalog.json`. Keep
the first two adjacent and include the generated module in an application or
in `zpl-bitmap-fonts`. Existing outputs are never overwritten.

The new `zpl_bitmap_fonts::collection` reader uses borrowed static arrays with
no allocation, dependencies or runtime JSON. It deduplicates glyph records,
bitmap payloads, map arrays and candidate-ID sets. It preserves 16-bit glyph IDs,
32-bit Unicode input keys, blank records, metric widths and padding ink. Existing `Font` and bundled `zd621` APIs are
unchanged; the renderer's bundled font behavior is not replaced by this work.

```rust,ignore
let font = generated::font_by_name("Z:EPL1.FNT").unwrap();
let raw = font.record(256).unwrap();
let map = font.encoding(zpl_bitmap_fonts::collection::Encoding::Input { ci: 28 }).unwrap();
if let Some(observation) = map.lookup(0x039b) {
    // Inspect status and every candidate; do not choose an ambiguous ID silently.
    println!("{:?}: {:?}", observation.status, observation.candidates);
}
```

Coverage is derived from the actual records and maps, protected by the collection
content hash. `unresolved_visible_record_ids` excludes only records matched by
observations; unverified candidates do not reduce it. Missing map keys are untested.
Collection JSON has a separate 256 MiB read/write bound, since complete multi-encoding
exports can exceed the legacy 32 MiB font-file bound. Oversized documents fail
before publication.

Array sizes in the compact catalog exclude static slice/pointer metadata and
Rust source text; they are not executable-size measurements.

## Validation

Tests cover Unicode inputs, duplicate glyph candidates, blank and unstable
previews, content-hash checks, mapping conflicts, provenance retention and JSON
size bounds. Generated modules are compiled and exercised through the compact
reader, including IDs above 255, signed bearings, wide glyphs and encoding maps.

The portable collection can preserve all supplied glyphs, including ones without
a measured input mapping. Coverage reports which records have been matched;
it cannot establish whether an external producer supplied every printer glyph.
