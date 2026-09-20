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

## ZBF1 and ZBF2 layouts

All multi-byte integers are little endian. Signed offsets are two's complement.

| Header field | Bytes |
| --- | --- |
| Magic `ZBF1` | 4 |
| Resident font ID (ASCII) | 1 |
| Requested height, requested width, DPI, glyph count (u16 each) | 8 |

Glyphs follow in ascending Unicode codepoint order. Each record has:

| Glyph field | Bytes |
| --- | --- |
| Codepoint | 1 |
| Advance (u16) | 2 |
| Left bearing, top relative to baseline (i16 each) | 4 |
| Bitmap width, bitmap height (u16 each) | 4 |
| Bitmap bits | `ceil(width × height / 8)` |

Bits are MSB-first, row-major, continuous across row boundaries. One is black.
Unused trailing bits are zero. Space has no bitmap but retains its advance.

ZBF2 uses the same layout with magic `ZBF2` and a four-byte little-endian
Unicode scalar value instead of the one-byte codepoint. Its count limit is
4096 glyphs; the existing two-MiB limit still applies. Surrogates and values
above U+10FFFF are rejected. The extractor emits ZBF1 when all characters fit
in one byte, preserving existing assets exactly, and ZBF2 otherwise. Sampling
with `--encoding 28` sends UTF-8 bytes and disables advanced text layout in
sampling pages so glyph metrics are measured without bidirectional reordering.
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

`fontE-28-15.zbf` supplies all 95 printable ASCII OCR-B glyphs, including the
ten unchanged digits used for UPC/EAN captions.
It is a native 203-DPI printer strike, with a 20-dot advance and zero-based
baseline 22. Raw sampling pages, extractor metadata and an independent exact
verification are preserved in
[retail-caption-zd621-v1](../tests/fixtures/retail-caption-zd621-v1/README.md).
The full strike replaces the original digit-only asset and enables general
resident E text. Its complete sampling, independent verification and asset
hash are in [resident-e-zd621-v1](../tests/fixtures/resident-e-zd621-v1/README.md).

`fontB-11-7.zbf` adds all 95 printable ASCII inputs for uppercase resident B,
with nine-dot advance and zero-based native baseline 10. Extraction pages,
metadata, an independent composition verification and the asset hash are in
[resident-bc-zd621-v1](../tests/fixtures/resident-bc-zd621-v1/README.md).
Font C reuses the captured D matrix, as specified by the ZPL Programming Guide
Table 31 (p. 1583) and verified by full-ASCII printer controls.

`fontF-26-13.zbf` contains all 95 printable ASCII inputs for resident F,
with 16-dot advance and zero-based native baseline 20. Raw extraction pages,
metadata, an independent exact composition verification and the asset hash
are preserved in
[resident-f-zd621-v1](../tests/fixtures/resident-f-zd621-v1/README.md).
Native dimensions, baseline and gap follow Tables 29/31 of the ZPL Guide
(pp. 1582–1583); magnified placement is checked against independent previews.

`fontG-60-40.zbf` contains all 95 printable ASCII resident G glyphs, with
48-dot advance and zero-based native baseline 47. Raw sampling uses four
glyphs per page; the 24 pages, independent exact verification, metadata and
asset hash are in
[resident-g-zd621-v1](../tests/fixtures/resident-g-zd621-v1/README.md).
Dimensions, gap and baseline follow Tables 29/31 of the ZPL Guide
(pp. 1582–1583); independent printer controls verify magnification and placement.

`fontH-21-13.zbf` contains all 95 printable ASCII inputs for resident H (OCR-A),
with 19-dot advance and zero-based native baseline 20. Space and lowercase
letters have empty bitmaps with their full advance, as measured on the printer.
Extraction, independent verification, asset hash and blank-versus-space controls
are in [resident-h-zd621-v1](../tests/fixtures/resident-h-zd621-v1/README.md).
Dimensions, gap and baseline follow Tables 29/31 of the ZPL Guide (pp. 1582–1583).

## Font 0 at 28 by 14 dots

`font0-28-14.zbf` adds a complete 95-character ASCII strike from the ZD621
203-DPI V93.21.33Z printer. The separate `font0-28-14-legacy-backslash.zbf`
preserves its CI0 backslash mapping. Both captures and their independent
verification strings matched exactly; repeated first pages were identical.
Raw requests, PNGs, asset hashes and regression comparisons live in
`../tests/fixtures/bounded-text-zd621-v1/`. This removes scaled-font errors in
the comparison corpus's TB cases; it does not claim other uncaptured sizes or
Unicode glyphs are exact.


`font0-40-24.zbf` adds a complete ASCII strike at that requested size;
`font0-40-24-extended.zbf` preserves é, cent, soft hyphen and eth support.
The `font0-{32-0,40-24}-hebrew.zbf` ZBF2 files contain all 27 Hebrew letters
and final forms. The 40×24 missing-character supplement records the native
blank advances of U+0378 and the five Arabic letters in the advanced-text
benchmark under `^PA0`; its ASCII A is a verification anchor, not an override.
The separate 40×24 legacy-backslash asset preserves native CI0/CI28 mapping.
All six assets and their sampling, repeated-control and independent preview
pages are pinned in `tests/fixtures/unicode-fonts-zd621-v1`.

The PA1 captures `font0-32-0-default-glyph.zbf` and
`font0-40-24-default-glyph.zbf` provide measured replacement boxes for U+0378
and Arabic U+0627/U+0628/U+062D/U+0631/U+0645. The matching
`font0-32-0-missing.zbf` records PA0 blank advances. ASCII A anchors their
independent verification strings and is not used as an override. Capture
with `extract-font --encoding 28 --default-glyph`; the selection is pinned in
the resumable capture configuration. Sources, raw previews, repeated pages,
verification strings and asset hashes are in `advanced-text-zd621-v1`.

## Preset P and font-0 comparison strike

`fontP-20-18.zbf`, `fontP-40-18.zbf`, and `fontP-40-36.zbf` contain 95 ASCII
characters each from the ZD621 preset P. `font0-20-18.zbf` fixes the mixed-font
control's comparison row. Raw requests, previews, capture metadata and exact
regeneration tests are documented in
[resident-p-zd621-v1](../tests/fixtures/resident-p-zd621-v1/README.md).


`fontQ-28-24.zbf` and `fontR-35-31.zbf` add the Q/R proportional presets,
with 95 ASCII glyphs each and measured baselines of 22 and 28. Raw native
sources, capture settings and offline regeneration tests are documented in
[resident-qr-zd621-v1](../tests/fixtures/resident-qr-zd621-v1/README.md).

`font0-40-22.zbf` fixes the 24 conformance anchor cases at that size.
[font0-40-22-zd621-v1](../tests/fixtures/font0-40-22-zd621-v1/README.md)
contains its 95-glyph source pages, independent verification and regression
controls; offline tests reproduce the asset byte-for-byte.

`font0-24-12.zbf`, `font0-20-10.zbf`, `font0-32-20.zbf` and `font0-26-16.zbf`
cover common text/field-block cases with independently hinted 95-character
ASCII strikes. Their raw sampling pages and offline regeneration tests are in
[font0-common-zd621-v1](../tests/fixtures/font0-common-zd621-v1/README.md).

`font0-28-14-hyphen.zbf` adds the two native automatic-hyphenation glyphs at
28x14. Raw sampling, independent verification and thirteen exact field-block
controls are in [font0-28-14-hyphen-zd621-v1](../tests/fixtures/font0-28-14-hyphen-zd621-v1/README.md).

`font0-24-16.zbf` and `font0-48-32.zbf` add 95-character ASCII strikes for
font defaults and per-field overrides. Raw sampling pages, independent
composition and rotated FO/FT controls are documented in
[font0-defaults-zd621-v1](../tests/fixtures/font0-defaults-zd621-v1/README.md).
Offline extraction tests reproduce both assets byte-for-byte.

`font0-10-0.zbf`, `font0-10-32.zbf` and `font0-32-10.zbf` capture the scalable
font's minimum dimensions. Each has 95 ASCII glyphs, raw sampling pages and an
offline regeneration test. See
[font0-minimum-zd621-v1](../tests/fixtures/font0-minimum-zd621-v1/README.md)
for threshold, baseline and origin controls.

Nine further font-0 ASCII strikes cover square requests at 15, 17, 31, 33, 63
and 65 dots, plus 64x16, 16x64 and 96x96. Their raw pages, independent
verification and regeneration tests are in
[font0-dimensions-zd621-v1](../tests/fixtures/font0-dimensions-zd621-v1/README.md).
Single-column pages keep the largest captures within the printer width.
