# Embedded resident font

`font0-32.zbf` contains 95 printable ASCII glyphs from resident font 0, sampled
from a Zebra ZD621 HTTP preview on 2026-09-14: height 32 dots, width 0 (natural
width), 203 DPI. Explicit width 32 was independently checked against width 0.
Capture and asset hashes are in `font0-32.provenance.json`. The original PNGs
and ZPL requests used by pixel-level regression tests are under
`../tests/fixtures/font0-32/`.

The asset is **4,365 bytes**, included with `include_bytes!` and decoded once.
No installed font, network access, JSON parsing, or filesystem lookup is required
at render time. The font has proportional advances, lowercase and descenders.
The renderer emits merged horizontal ink runs as paths for both SVG and PNG.
Normal text at the captured size is exact against the saved previews. Scaling,
other DPI values, and rotation are explicitly marked as approximations.

## Regenerate

From the repository root, using a fresh output directory:

```sh
direnv exec . cargo run -p zebra-http-api --example extract-font -- \
  --host http://printer.local/ --font 0 --height 32 --width 0 --dpi 203 \
  --verify-text 'AVATAR Agj Wavy 123 _^~|!' _font-0-32
cp _font-0-32/font.zbf zpl/assets/font0-32.zbf
direnv exec . cargo test -p zpl --lib --test render
```

The extractor always exports ZBF alongside JSON and BDF. Existing captures can
be repacked with `--offline`. Keep the provenance hashes and reference fixtures
consistent when intentionally changing the embedded strike. A subset capture is
not a replacement for this complete 95-glyph asset.

## ZBF1 layout

All multi-byte integers are little endian. Signed offsets are two's complement.

| Header field | Bytes |
| --- | --- |
| Magic `ZBF1` | 4 |
| Resident font ID (ASCII) | 1 |
| Requested height, requested width, DPI, glyph count (u16 each) | 8 |

Glyphs follow in ascending ASCII codepoint order. Each record has:

| Glyph field | Bytes |
| --- | --- |
| Codepoint | 1 |
| Advance (u16) | 2 |
| Left bearing, top relative to baseline (i16 each) | 4 |
| Bitmap width, bitmap height (u16 each) | 4 |
| Bitmap bits | `ceil(width × height / 8)` |

Bits are MSB-first, row-major, continuous across row boundaries. One is black.
Unused trailing bits are zero. Space has no bitmap but retains its advance.
This avoids JSON text and row-padding overhead while retaining exact metrics.

`zpl_font_extract::pack` and `zpl::bitmap_font::unpack` implement the format, with dimension,
ordering, length and padding validation. Runtime `font 0` scaling uses 32 as the
native width/height and the documented baseline of three quarters of height;
this embedded asset's metadata and completeness are checked in unit tests.

## Size and rotation study

See the [font reconstruction study](../../docs/font-reconstruction.md) for live captures across 14 size
configurations, all four rotations, larger-strike scaling, and the assessment
of outline and hint-parameter fitting.

## Additional resident strikes

`font0-16-0`, `font0-20-0`, `font0-64-0`, and `font0-32-{16,24,64}`
provide complete printable ASCII strikes at those requested height/width pairs.
`fontA-9-5` and `fontD-18-10` contain native bitmap matrices, enlarged by integer
multipliers. `font0-32-latin1` supplements the natural-width 32-dot strike with é.
Capture provenance, source/image hashes, asset hashes and regression controls
are in `tests/fixtures/resident-fonts-zd621-v1`. ZBF1's one-byte codepoint also
supports U+00A0–00FF; counts are bounded at 191 and C0/C1/DEL remain excluded.
Unsampled font-0 sizes still fall back to scaling the original 32-dot strike.
