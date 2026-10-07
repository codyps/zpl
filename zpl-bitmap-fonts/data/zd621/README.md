# ZD621 glyphs and encodings — 2026-10-06 snapshot

This portable v3 dataset combines existing glyph exports and encoding observations
for the ZD621 at 203 dpi, software version V93.21.33Z. It is not a fresh complete
run of the unified prober, nor a claim about other printers or versions.

- 47 fonts, 10,111 glyph records, including 8,162 visible records.
- Includes 11 additional advancing blank records and calibrated cell metrics from
  the previously verified source-position captures.
- 6,872 visible records have measured input equivalents; 1,290 remain unresolved.
- Unverified candidates cover 1,122 of those unresolved records.
- 61 imported byte-to-character tables remain explicitly unverified.

Mappings preserve exact tested inputs, duplicate candidates, unresolved outcomes
and separate source-byte/Unicode/candidate-character domains. Candidate evidence
does not establish measured printer behavior or reduce unresolved coverage.

The JSON retains original glyph source hashes, observation/report digests and
hashes of original provenance documents. Raw captures and those original documents
are not bundled here. Importing and validating a content hash establishes integrity,
not independent authentication of every observation. Location-specific annotations
are excluded from this portable snapshot.

The JSON content SHA-256 is:

`20da33bb37bb031ab410552097de994d365a724c77df06cf55bb66fee132a81c`

Regenerate the matching `src/zd621` artifacts into a new directory:

```sh
cargo run --locked -p zpl-font-extract -- compile \
  zpl-bitmap-fonts/data/zd621/fonts.json --out _zd621
```

Compare all three outputs (`fonts.rs`, `bitmaps.bin`, `catalog.json`) before
replacing tracked artifacts. The `bundled_encodings` integration test checks every
glyph/metric/pixel and encoding/status/candidate/code-page entry against the JSON,
and requires byte-for-byte reproduction of the generated artifacts.

The `zd621` feature exposes `zpl_bitmap_fonts::zd621`. This is the single bundled
font dataset, also used by the renderer. Its CI0 source glyph views crop the shared
padded records without allocating or duplicating bitmaps. Only measured mappings
supply renderer glyphs; unverified candidates remain explicitly separate.

The original 45-font source-position behavior is protected by the unchanged
whole-dataset pixel/metric golden digest in `tests/bundled.rs`. Font content retains
its original licensing.
