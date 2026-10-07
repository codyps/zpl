# ZD621 preview-derived glyphs and encodings

This portable v4 dataset contains records with measured preview equivalents for
the ZD621 at 203 dpi, software version V93.21.33Z. It makes no claim about other
printers or versions. Character tables and records without observed equivalents
are not bundled.

The completed 2026-10-07 preview sweep contains 47 fonts and 8,831 records, including
6,882 visible records. It preserves calibrated native metrics and independently
measured advancing blanks. This is a measured repertoire, not proof that every
possible printer glyph is reachable or recovered.

Every font covers all 256 inputs in 21 byte/source domains and the documented
bounded CI28 Unicode repertoire. Blank and unmatched observations remain unresolved.
The directory audit additionally tested 22 selectors: default-font fallback or
non-native scaling prevented treating them as recovered bitmap fonts. Their
inspection reports and preview hashes are retained in provenance.

Mappings retain exact tested inputs, equivalent record IDs, unresolved outcomes,
and separate source-position and Unicode input domains. A blank observation alone
cannot establish a glyph or its advance. Raw records may share identical pixels;
preview equivalence does not identify one physical record.

Source/reference hashes and observation digests retain the lineage of existing
verified captures. Hashes establish integrity, not independent authentication.
Raw PNG/ZPL evidence is kept by the recovery tool, outside the crate package.

JSON content SHA-256: `60f0a27d354b9c18c7675c1cee561f91c8d9027b5da2a96af315af7d4e6b6dae`.

Regenerate the matching `src/zd621` artifacts into a new directory:

```sh
cargo run --locked -p zpl-font-extract -- compile \
  zpl-bitmap-fonts/data/zd621/fonts.json --out _zd621
```

The `bundled_encodings` integration test checks every record, metric, pixel,
encoding input and observation against JSON and requires byte-for-byte artifact
reproduction. The original 45-font source-position behavior retains its existing
golden digest. The `zd621` feature exposes the single dataset used by the renderer.
