# ZD621 glyphs and encodings — 2026-10-06 snapshot

This portable v3 dataset combines existing glyph exports and encoding observations
for the ZD621 at 203 dpi, software version V93.21.33Z. It is not a fresh complete
run of the unified prober, nor a claim about other printers or versions.

- 47 fonts, 10,100 glyph records, including 8,162 visible records.
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

`760662ac0d7855547f02c9805e18c66b8613a267aa0f473d7b11659ae0c5a8bd`

Regenerate the matching `src/zd621_encodings` artifacts into a new directory:

```sh
cargo run --locked -p zpl-font-extract -- compile \
  zpl-bitmap-fonts/data/zd621-encodings/fonts.json --out _zd621-encodings
```

Compare all three outputs (`fonts.rs`, `bitmaps.bin`, `catalog.json`) before
replacing tracked artifacts. The `bundled_encodings` integration test checks every
glyph/metric/pixel and encoding/status/candidate/code-page entry against the JSON,
and requires byte-for-byte reproduction of the generated artifacts.

Enable `zpl-bitmap-fonts` feature `zd621-encodings` to access
`zpl_bitmap_fonts::zd621_encodings`. This opt-in dataset uses the `collection`
reader. The existing `zd621` feature and renderer dataset remain unchanged.
Font content retains its original licensing.
