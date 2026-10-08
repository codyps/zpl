# Captured resident font provenance

All 101 scalable captures are now represented in
[`captures.json`](../../zpl-bitmap-fonts/data/captures.json) and compiled into
shared records, bitmap bytes and compatibility maps in `zpl-bitmap-fonts`.
Names ending in `.zbf` below are historical capture identities, not runtime
files. The original hashes remain pinned; `compact_captures` reconstructs all
5,710 glyphs into the historical encoding and checks every original hash.
Native A–H/GS and scalable 0/P–V now use the same compact glyph view.

The ZD621 evidence applies only to 203 dpi and firmware V93.21.33Z. No new
printer campaign or additional glyph coverage is claimed by this migration.

`font0-32.zbf` contains 95 printable ASCII glyphs from resident font 0, sampled
from a Zebra ZD621 HTTP preview on 2026-09-14: height 32 dots, width 0 (natural
width), 203 DPI. Explicit width 32 was independently checked against width 0.
Capture and asset hashes are in `font0-32.provenance.json`. The original PNGs
and ZPL requests used by pixel-level regression tests are under
`../tests/fixtures/font0-32/`.

The historical base asset was **4,365 bytes**. Rendering now borrows compiled
glyph records and bitmap slices without decoding or cloning strikes.
No installed font, network access, JSON parsing, or filesystem lookup is required
at render time. The font has proportional advances, lowercase and descenders.
The renderer emits merged horizontal ink runs as paths for both SVG and PNG.
Normal text at the captured size is exact against the saved previews. Scaling,
other DPI values, and rotation are explicitly marked as approximations.

## Regenerate

Run the extractor from a separate checkout of the
[private font research repository](https://github.com/codyps/zpl-font-extract),
using a fresh output directory:

```sh
cargo run --locked -p zebra-http-api --example extract-font -- \
  --host http://printer.local/ --font 0 --height 32 --width 0 --dpi 203 \
  --verify-text 'AVATAR Agj Wavy 123 _^~|!' _font-0-32
```

Import verified capture metrics into the fixed JSON corpus, retaining provenance
and independent regression fixtures. Generate and check the runtime tables with:

```sh
python3 zpl-bitmap-fonts/data/compile_captures.py
python3 zpl-bitmap-fonts/data/compile_captures.py --check
cargo test --locked -p zpl --test compact_captures
```

The compiler only packs this fixed corpus; extraction, sampling, fitting and
new research datasets remain in the private font research repository. A subset
capture is not a replacement for a complete 95-glyph ASCII strike.

## Historical ZBF1 and ZBF2 layouts

All multi-byte integers are little endian. Signed offsets are two's complement.

| Header field | Bytes |
| --- | --- |
| Magic `ZBF1` | 4 |
| Resident font ID (ASCII), or `@` for graphic symbols | 1 |
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

ZBF2 used the same layout with magic `ZBF2` and a four-byte little-endian
Unicode scalar value instead of the one-byte codepoint. The historical exporter
selected ZBF1 for one-byte characters and ZBF2 otherwise.

These layouts are retained here only to explain original capture hashes.
`tests/support/compact_font.rs` reconstructs the historical byte stream from
compact glyph data to check those hashes. The library no longer decodes either
format; runtime registration accepts decoded glyphs or a `fonts::BitmapFont`
provider. Bundled fonts use shared compiled tables, not serialized ZBF files.

## Size and rotation study

See the [font reconstruction study](https://github.com/codyps/zpl-font-extract/blob/main/docs/font-reconstruction.md) for live captures across 14 size
configurations, all four rotations, larger-strike scaling, and the assessment
of outline and hint-parameter fitting.

## Additional resident strikes

`font0-16-0`, `font0-20-0`, `font0-64-0`, and `font0-32-{16,24,64}`
provide complete printable ASCII strikes at those requested height/width pairs.
Native bitmap faces A–H and GS are supplied by the
[`zpl-bitmap-fonts` crate](../../zpl-bitmap-fonts/README.md); their former ZBF
assets and supplements have been removed. `font0-32-latin1` supplements the
natural-width 32-dot strike with é.
Capture provenance, source/image hashes, asset hashes and regression controls
are in `tests/fixtures/resident-fonts-zd621-v1`. ZBF1's one-byte codepoint also
supports U+00A0–00FF; counts are bounded at 191 and C0/C1/DEL remain excluded.
Unsampled font-0 sizes still fall back to scaling the original 32-dot strike.

`font0-28-0.zbf` adds all 95 printable ASCII glyphs at natural-width 28 dots.
The 3,584-byte strike, raw sampling pages, exact independent text verification,
four-orientation FO/FT controls, and full Labelixa QR label comparisons are
documented in [qr-segmentation-zd621-v1](../tests/fixtures/qr-segmentation-zd621-v1/README.md).

The compact bitmap collection supplies A–H and GS, including native baselines,
advances, CI0 source positions, and the distinct Unicode/legacy backslash designs.
C aliases D; E and H select the 203-dpi E8/H8 faces. Printer-capture fixtures and
historical asset hashes remain in `tests/fixtures/resident-*-zd621-v1` and
`graphic-symbols-zd621-v1`. Tests reconstruct the original ASCII strike encoding
from the compact reader and verify those hashes, without retaining duplicate
font assets.


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

`font0-16-10.zbf` is a 95-character ASCII strike sampled from the ZD621 at
16x10 dots. Raw pages and independent verification are in
`tests/fixtures/long-field-zd621-v1`; the extraction test reproduces every byte.

Native ASCII strikes `fontT-48-42.zbf`, `fontU-59-53.zbf` and `fontV-80-71.zbf`
are reproduced from `tests/fixtures/resident-tuv-zd621-v1`. The independent
origin atlas measures their baselines; these are distinct resident faces.

`fontS-40-35.zbf` and `fontS-80-70.zbf` are native resident-S ASCII strikes.
These scalable-font strikes remain separate from GS, which now comes from
`zpl-bitmap-fonts` under the explicit `@` resident alias. Printer-capture
regressions continue to distinguish the two faces.
