# Complete bitmap records and encoding mappings

`zpl-font-extract collection` adds a stored-font and mapping pipeline alongside
the existing preview-only `recover` command. Font glyphs and input encodings
are separate: a 16-bit raw FNT record ID is not a Unicode code point, and an
encoding observation may match several indistinguishable records.

The v2 JSON can represent every record in a supported stored bitmap FNT,
including records that previews cannot currently select. It does **not** claim
that every encoding mapping can be discovered through ZPL previews. Tested
inputs, unresolved inputs, duplicate candidates and untested inputs remain
separate. UTF-16 HTTP previews remain disabled following the ZD621 hang recorded
in the [firmware investigation](https://github.com/codyps/zebra-firmware/blob/768c231455191670dff1ac52872a8f42fb6d350a/docs/bitmap-encoding-recovery.md).
Firmware disassembly and investigation belong in that repository; this pipeline
can import its exported candidate evidence without treating it as a printer test.

## Read the stored fonts

The inventory command queries `file.dir` and reads the listed `.FNT` objects
through SGD. It uses the observed trailing-dot `file.type` form for protected
resident files. It validates complete directory framing, expected object sizes
and FNT bounds, saves originals and hashes, and rejects truncated responses.
It sends no physical print, configuration, download or firmware-update commands.
The default drive is `Z`; use `--drives EZ` to include downloaded fonts.

```sh
cargo run --locked -p zpl-font-extract -- collection inventory \
  --printer printer.example:9100 --drives Z --out _font-inventory
```

The decoder supports the observed type-1 bitmap FNT representation. Descriptor,
outline and other non-bitmap FNT files are retained and listed as skipped;
TrueType font files are outside this bitmap-record pipeline. A malformed type-1
font fails collection rather than silently disappearing. Failed downloads leave
raw evidence but no successful `fonts.json`.

Existing archived files named `Z-A.FNT`, `Z-EPL1.FNT`, etc. can be imported offline:

```sh
cargo run --locked -p zpl-font-extract -- collection import ARCHIVED_FNT_DIRECTORY \
  --out _font-inventory/fonts.json
```

This exports the complete slot inventory: decoded records, absent slots and
zero-record slots. Record IDs extend beyond 255. Complete padded bitmap rows,
including padding ink, signed bearings, advance, flags and original headers
are retained. Original file hashes link records to their source objects.

## Obtain input mappings

```sh
cargo run --locked -p zpl-font-extract -- collection survey \
  _font-inventory/fonts.json --host http://printer.example/ \
  --cache _font-cache --out _font-mapped.json
```

By default the survey visits all collected bitmap fonts, all 256 inputs for CI0
source remapping and CI0–13/27/31/33–36, and a bounded UTF-8 discovery repertoire:
U+0000–052F, U+2000–26FF, U+F000–F0FF and U+FFFD–FFFF. This is not an exhaustive
Unicode or multibyte-code-page sweep. `--font`, `--encoding`, and `--codes` select
focused work; all keys are decimal. For example:

```sh
cargo run --locked -p zpl-font-extract -- collection survey \
  _font-inventory/fonts.json --host http://printer.example/ \
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
  _font-inventory/fonts.json \
  --report zpl-bitmap-fonts/data/zd621/fonts.json \
  --report PATH_TO_ENCODING_OBSERVATIONS.json \
  --report PATH_TO_LEGACY_FONT_MAPS.json \
  --evidence-root PATH_TO_ZEBRA_FIRMWARE_WORKTREE \
  --out _font-mapped.json
```

Accepted evidence formats are the existing verified `zebra-bitmap-fonts` v1
collection, `zebra-font-encoding-observations-v1`, and
`zebra-legacy-firmware-font-maps-v1`. The v1 import checks its verification/content
hash and compares actual glyph pixels and advance against stored records. The
observation import checks reference-font hashes and the saved plan, PNG/ZPL,
repeat and completion receipts. Conflicting observations fail rather than
silently replacing earlier results. Experimental out-of-range source mappings
are not accepted.

Firmware candidate mappings have their own namespace and status. Their raw
code-page tables retain every byte and the original firmware character keys,
including vendor-specific keys. The compact reader only composes these tables
with firmware candidate maps through an explicitly named `candidate()` method.
They never become measured CI mappings automatically.

## Compile for zpl-bitmap-fonts

```sh
cargo run --locked -p zpl-font-extract -- collection compile \
  _font-mapped.json --out _compiled-fonts
```

The compiler validates schema, content hash, complete slot coverage, record
bounds, candidate references, mapping domains and evidence categories before
writing anything. It emits `fonts.rs`, `bitmaps.bin`, and `catalog.json`. Keep
the first two adjacent and include the generated module in an application or
in `zpl-bitmap-fonts`. Existing outputs are never overwritten.

The new `zpl_bitmap_fonts::collection` reader uses borrowed static arrays with
no allocation, dependencies or runtime JSON. It deduplicates glyph records,
bitmap payloads, map arrays and candidate-ID sets. It preserves 16-bit raw IDs,
32-bit Unicode input keys, original headers, absent/zero/blank distinctions,
metric widths and padding ink. Existing `Font` and bundled `zd621` APIs are
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
observations; firmware candidates do not reduce it. Missing map keys are untested.
Collection JSON has a separate 256 MiB read/write bound, since complete multi-encoding
exports can exceed the legacy 32 MiB font-file bound. Oversized documents fail
before publication.

Array sizes in the compact catalog exclude static slice/pointer metadata and
Rust source text; they are not executable-size measurements.

## Validation on the existing archive

The Rust importer decoded all 47 bitmap fonts and 10,100 records (8,162 visible)
from the ZD621 203-dpi/V93.21.33Z archive. Importing the existing CI0 dataset,
saved Unicode/byte surveys, and firmware evidence reproduces 6,872 observed
record equivalents and 1,290 unresolved records. Candidate evidence covers
1,122 of those unresolved records; it remains unverified on the current printer.
All 61 legacy byte-code-page tables were retained.

The generated module was compiled and every record, header, metric, padding
byte, mapping entry/status/candidate and code-page entry was read back through
the allocation-free API and compared byte-for-byte with the JSON. The resulting
arrays occupy 1,600,335 bytes versus 27,695,316 bytes for the pretty-printed JSON.
A focused live Font A survey exercised source mapping, CI27 and UTF-8 with
native preview repeats; its cached run was also replayed offline.

The Rust inventory command also downloaded all 69 resident `.FNT` objects
from the printer: the 47 bitmap fonts and all their records were byte-identical
to the archive; 22 non-bitmap descriptors were retained and explicitly skipped.
Final targeted validation: 23 tests passed, with Clippy (`-D warnings`) and
workspace rustfmt checks passing. The identity-scoped live survey and its
offline replay produced identical JSON.
