# Constructed-font results: 2026-10-02

**Use scalable outlines and execute their hints at the requested X/Y sizes, then
apply measured spacing and monochrome scan-conversion rules.** This campaign
supports that direction without collecting another resident-font strike at each
size. It also shows why installing FreeType with its default monochrome settings
would not yet reproduce the printers.

An original 3,540-byte TrueType font was uploaded to two development printers.
All 19 constructed-font pages were pixel-identical between the printers, at their
original canvas origins. These are downloaded-font observations, not a claim of
resident Font 0 parity or compatibility with other firmware.

| Printer | Serial | Firmware | Resolution |
| --- | --- | --- | --- |
| ZD621 | D7J211001302 | V93.21.33Z | 203 DPI |
| ZQ610 Plus | XXZMJ230802993 | V100.21.21Z | 203 DPI |

The [fixtures and reproduction commands](../zpl-font-extract/tests/fixtures/font-probes-20261002/README.md)
include the constructed font, source manifest, exact submitted ZPL, raw PNGs,
hashes, capture records, and [complete analysis](../zpl-font-extract/tests/fixtures/font-probes-20261002/analysis.json).
The font contains 59 glyphs including `.notdef`, with 2048 units/em. Its contours
and instructions are original; ancillary defaults come from the repository's
earlier original calibration font. No external resident-font file was used.

## Hint execution and independent dimensions

One glyph uses `SCFS` to force a 13-dot-wide bar. Two others encode the value of
`MPPEM`, queried along X or Y, as their bar widths. Distinctive geometry verifies
font selection; the unconditional 13-dot bar verifies instruction execution.
All sizes and orientations below retained the 13-dot width along the glyph's
local X axis.

| Requested height × width | Orientation | X ppem | Y ppem | GETINFO stretch |
| --- | --- | ---: | ---: | --- |
| 16 × natural | N | 16 | 16 | false |
| 32 × natural | N | 32 | 32 | false |
| 64 × natural | N | 64 | 64 | false |
| 32 × 19 | N | 19 | 32 | true |
| 32 × 64 | N | 64 | 32 | true |
| 32 × 19 | R | 19 | 32 | false |
| 32 × 19 | I | 19 | 32 | true |
| 32 × 19 | B | 19 | 32 | false |

The tested rotation and grayscale flags were false in every case. These are
interpreter-reported flags; they do not identify an engine vendor or establish
the complete internal transform order. FreeType reproduced the X/Y ppem and
fixed-width witnesses, but returned false for the three stretch-flag cases above.

The combination of reported ppem and fixed device-width geometry rules out
hinting at one isotropic size followed by a simple X stretch for these probes.
A candidate font API must support the two hinting dimensions, rather than merely
offer an affine transform on its final paths. See the Microsoft definitions of
[MPPEM and GETINFO](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).

## Spacing is a separate policy

Four glyphs have identical contours: `hmtx` advances 735, 736, and 737 design units,
plus a 735-unit variant whose instructions move the advance-defining phantom
point by one device dot. Visible sentinels measure strings containing one or four
copies, with both `^FO` and `^FT` origins. All four combinations of origin and
repetition count agreed:

| Height | Printer: all four variants | FreeType: 735 | FreeType: 736 / 737 | FreeType: 735 + phantom-point shift |
| --- | ---: | ---: | ---: | ---: |
| 16 | 6 | 6 | 6 | 7 |
| 32 | 12 | 11 | 12 | 12 |
| 64 | 23 | 23 | 23 | 24 |

The instruction changes FreeType's advance by exactly one dot at all three sizes,
so the probe is executable and its phantom-point index is valid. It did **not**
change the observed printer spacing. This rules out blindly using FreeType's
hinted advance for these downloaded-font fields. It does not prove that the
printer ignores every possible metric instruction, or that resident Font 0 uses
the same policy.

At 32 dots, 735 units scale to 11.484375 dots, yet the printer advances 12. The
three adjacent advances do not identify where that extra quantization/rounding
occurs. A compact follow-up should sweep advance values around this transition
and independently vary side bearings, explicit widths, and font-table flags.
Avoid adding a per-size correction before that rule is understood.

## Edge, dropout, and transform behavior

The phase family samples 0, 16, 31, 32, 33, 48, and 63 design units. At 32 ppem
these are exact increments of 1/64 dot. Wide bars are two dots thick; thin bars
are half a dot thick. There are also diagonals, counters, overlapping contours,
stubs, and whole/subdivided quadratic curves.

Concrete observations at 32 dots, natural width:

- A vertical bar with X edges at 4.5 and 6.5 dots occupies columns 4–5 on the
  printer and 4–6 in FreeType. A horizontal bar with Y edges at 2.5 and 4.5 has
  one printer row in N orientation and three FreeType rows. This is axis-sensitive
  boundary behavior, not an outline mismatch.
- Thin vertical bars at phases 33, 48, and 63 remain visible on the printer but
  disappear in the tested FreeType mode. Some printer stems omit their endpoint
  pixels. An unconditional minimum-width expansion would not explain all cases.
- Rendering the N page once and rotating its finished glyph bitmaps to the
  declared R/I/B positions leaves respectively **297 / 363 / 220 differing
  pixels**. These comparisons use exact prescribed integer transforms and full
  target canvases, with no searched alignment. Rotation must remain part of the
  rasterization experiment and cache key.
- Whole and exactly subdivided representations of a quadratic differ by **one
  printer pixel** at phase 0 in N orientation. The other 14 paired observations
  agree. Rational arithmetic verifies the encoded curves are mathematically
  identical. Subdivision/precision behavior therefore deserves its own model;
  changing control points to hide this error would be misleading.
- Integral overlap and counter cases agree with FreeType in all four natural-width
  orientations. At width 19, the counter still agrees but the overlap has eight
  differing pixels. These controls do not establish every fill-rule or
  contour-order case.

The relevant specification is the
[TrueType scan converter](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter).
These observations do not yet select a complete replacement scan converter.

## FreeType comparison

FreeType 2.13.2 uses `FT_LOAD_RENDER | FT_LOAD_TARGET_MONO | FT_LOAD_NO_AUTOHINT`,
independent pixel sizes, the declared rotation, and a fresh face for every glyph.
The comparison covers whole native canvases without alignment, cropping,
padding, or resampling. Counts below are **per printer**; both have the same result.

| Group | Missing pixels | Extra pixels | XOR | Foreground IoU | Exact ink cases |
| --- | ---: | ---: | ---: | ---: | ---: |
| Hint-state markers | 48 | 0 | 48 | 99.012346% | 53/56 |
| Geometry probes | 1,179 | 647 | 1,826 | 88.491838% | 175/317 |

| Geometry setting, height × width | Orientation | XOR | Exact ink cases |
| --- | --- | ---: | ---: |
| 32 × natural | N | 225 | 28/46 |
| 32 × natural | R | 322 | 24/46 |
| 32 × natural | I | 322 | 23/46 |
| 32 × natural | B | 231 | 26/46 |
| 31 × natural | N | 106 | 11/29 |
| 33 × natural | N | 128 | 21/29 |
| 64 × natural | N | 256 | 21/29 |
| 32 × 19 | N | 236 | 21/46 |

The geometry set deliberately concentrates on difficult boundaries and thin
features. Its 88.49% IoU is not an estimate of ordinary text quality and is not
comparable to the earlier 97.58% aggregate on a different probe set. Metrics
fields are measured separately; they are not counted as full-image FreeType
parity tests. All data are development evidence, with no new sealed holdout claim.

## Capture issues and cleanup

Each completed run used 19 probe pages and identical resident-font start/end
controls. The repeated resident controls match exactly within each printer.
Total: **43 HTTP previews**, including one retained diagnostic failure, and two
font uploads. Formats were submitted through HTTP preview; no physical print,
firmware update, or power cycle was requested.

The initial ZQ610 control requested 384×192 but returned 384×2030. Its mark-media
configuration had `zpl.label_length_always=no`. The diagnostic PNG is retained
unchanged and excluded from parity results. The corrected run used
[`^LLh,Y`](https://docs.zebra.com/content/tcm/us/en/printers/software/zpl-pg/zpl-commands/%5Ell.html),
which applies the requested height to mark/gap media. It restored and verified
the original `zpl.label_length_always=no` and `zpl.label_length=2030` afterwards.
The prepared and actually submitted request hashes are both recorded.

The ZD621's initial HTTP delete request did not remove the temporary RAM font.
Its original failed cleanup record is preserved; a separate identity-checked
recovery record confirms removal with
[SGD `file.delete`](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-sgd-commands-from-a-to-d/r-sgd-file-delete.html).
The ZQ610 run used that command directly. `R:ZP26A.TTF` was confirmed absent on
both printers. No existing font object was overwritten or deleted.

## Consequence for arbitrary-size rendering

Prioritize the original Font 0 outlines and programs if the external TTF becomes
available, as described in [the options assessment](font-scaling-options.md).
Use FreeType as an offline reference and diagnostic engine; its unmodified
metrics and monochrome rasterizer are not yet a printer-parity backend. A Rust
engine must additionally demonstrate independent X/Y hinting before selection.

For an original implementation, these fixtures now provide executable tests for
a bounded TrueType interpreter and a shared scan converter. Resolve half-dot
edge inclusion, directional dropout, and curve precision before fitting more
outlines. Determine advance rounding independently of glyph ink. Then generate
monochrome coverage on demand at any requested dimensions and emit the existing
scene's pixel runs. A bounded glyph cache avoids repeated work without requiring
captured assets or separately tuned parameters for every size.

No production renderer or baseline was changed. The external Font 0 asset is
still absent locally, and equivalence between downloaded and resident font paths
remains untested. A reconstructed candidate uploaded to the printer would next
separate candidate-font error from local-engine error; it should follow calibration
and then be judged on independent sizes, aspect ratios, rotations, and strings.
