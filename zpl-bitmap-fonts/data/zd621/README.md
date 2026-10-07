# ZD621 preview-derived glyphs and encodings

This portable v4 dataset contains records with measured preview equivalents for
the ZD621 at 203 dpi, software version V93.21.33Z. It makes no claim about other
printers or versions. Character tables and records without observed equivalents
are not bundled.

The initial preview-only subset contains 47 fonts and 8,821 records, including
6,872 visible records. It preserves calibrated native metrics and independently
measured advancing blanks. This is a measured repertoire, not proof that every
possible printer glyph is reachable or recovered.

Mappings retain exact tested inputs, equivalent record IDs, unresolved outcomes,
and separate source-position and Unicode input domains. A blank observation alone
cannot establish a glyph or its advance. Raw records may share identical pixels;
preview equivalence does not identify one physical record.

Source/reference hashes and observation digests retain the lineage of existing
verified captures. Hashes establish integrity, not independent authentication.
Raw PNG/ZPL evidence is kept by the recovery tool, outside the crate package.

JSON content SHA-256: `7c6e64ef98de35130315ba9b65fc9b8734561f1fd7d76ade491590cc4c768e6d`.

Regenerate the matching `src/zd621` artifacts into a new directory:

```sh
cargo run --locked -p zpl-font-extract -- compile \
  zpl-bitmap-fonts/data/zd621/fonts.json --out _zd621
```

The `bundled_encodings` integration test checks every record, metric, pixel,
encoding input and observation against JSON and requires byte-for-byte artifact
reproduction. The original 45-font source-position behavior retains its existing
golden digest. The `zd621` feature exposes the single dataset used by the renderer.
