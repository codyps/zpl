# ZD621 TrueType contour witnesses

This fixed selection comes from preview captures on October 10, 2026 using the
ZD621 at 203 DPI, serial D7J211001302, firmware V93.21.33Z. Every page was captured
twice with byte-identical PNGs; a box control before and after each session was
also byte-identical. Each font was uploaded under a checked, unused RAM name and
removed afterward. Capture ledgers preserve identity, request hashes, PNG hashes,
repetitions and successful cleanup. No physical printing was performed. An
earlier exploratory large-value probe hung the preview service; a JSON SGD reset
on port 9200 recovered it. All sessions retained here completed independently
with matching controls. The failed session and recovery evidence are preserved
in the private research repository, not treated as correctness evidence.

`index.json` hashes all retained original inputs, fonts, manifests, ledgers and
native PNGs. Manifests and ledgers are unchanged, including entries for captures
outside this fixed selection. The index identifies the retained pages and pins
full-canvas pixel counts and candidate hashes. Tests compare the original
832-by-832 canvases without alignment, padding, cropping or rescaling, using
`render::profiles::ZD621_203_DPI`. They make no printer requests.

The constructed fonts are original test data. SCFS sets exact 26.6 coordinates;
GC[1] and RCVT expose original-point and control-value scaling as bar widths.
Instructions follow the [OpenType TrueType instruction specification](https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions).
Terminology for winding and dropout follows the [scan converter rules 1–4](https://learn.microsoft.com/en-us/typography/opentype/spec/ttch01#the-scan-converter).
The measured compatibility rules are not requirements of those specifications.

| Group | Pages / fields | Purpose | Under / over |
| --- | ---: | --- | ---: |
| curves | 4 / 256 | Independently reserved fractional quadratic contours | 0 / 0 |
| lines | 4 / 256 | Matching straight-edge controls | 0 / 0 |
| thresholds | 2 / 128 | Curvature norm, uniform subdivision and midpoint rounding | 0 / 0 |
| flat-ties | 1 / 64 | Exact initial flatness threshold at different subpixel phases | 0 / 0 |
| scale-1024 | 2 / 160 | Intermediate ppem and multiplier rounding, 32/64 dots | 0 / 0 |
| scale-2000 | 2 / 80 | GC/RCVT controls at 16/32 dots, including rational half-ties | 0 / 0 |
| scale-binary | 11 / 504 | All binary ppem candidates through 1500 dots, plus adjacent sizes | 0 / 0 |
| scale-wide-2000 | 3 / 144 | Reserved 1014/1015/1016-dot probes, including 32-bit wrap | 0 / 0 |
| scale-wide-999 | 2 / 96 | Integral-denominator requirement at 127/1015 dots | 0 / 0 |
| scale-wide-1024 | 2 / 96 | Signed shifts and large coordinates at 127/1015 dots | 0 / 0 |
| scale-ties-1000 | 1 / 48 | Reserved signed division and fixed-multiplier half-ties | 0 / 0 |
| scale-ties-1024 | 1 / 48 | Reserved signed-shift and fixed-multiplier half-ties | 0 / 0 |
| scale-ties-16384 | 1 / 48 | Reserved large-em signed-shift half-ties | 0 / 0 |

All 36 pages / 1,928 fields are exact. The original 16-dot scale-2000 page improved
from 128 underpaint / 500 overpaint pixels to zero, without changing its captured
bytes. Fractional witnesses magnify a one-tick difference into a bar, and a wrap
from fraction 0 to 63 magnifies it further; these are not normal-text error
percentages. The production rule has no font-name, glyph or strike exceptions.
Additional exploratory captures and scripts belong in the private
[zpl-font-extract repository](https://github.com/codyps/zpl-font-extract).

## Scaling arithmetic

Let `p = floor(max(dots, 10) * 1152 / 203)` be the size in sixteenths of a point.
When `q = p / 45` is an integral power of two and `D = unitsPerEm * 32 / q` is
integral, the measured rational path uses numerator `508 * 32 = 16256`:

- For power-of-two `D`, add `D/2` to the signed product and shift right. Negative
  half-ties round toward positive infinity.
- Otherwise, multiply the magnitude, add `floor(D/2)`, divide with truncation
  toward zero, then restore the input sign. Ordinary half-ties round away from
  zero.
- Both the multiplication and rounding addition wrap at 32 bits. Bounded WCVTF
  witnesses expose the wrap without emitting enormous outlines.

Otherwise, round the ppem to 16.16, round its 26.6-per-FUnit multiplier to 16.16,
and round each scaled magnitude before restoring its sign. Merely using rational
arithmetic for every exactly representable ppem fails the 119/238-dot controls
and the exhaustive binary-ppem sweep. At 1015 dots, odd em sizes also require the
fixed-multiplier path because `D` would be fractional. The reserved half-tie
pages confirm that its negative ties round away from zero.

These are empirical rules for the recorded firmware, not claims about its source
code. Original-point GC[1], initial RCVT and WCVTF probes agree. Full native
canvases retain their origins; no image registration or resampling is used.

## Findings

Aspect projection rounds in font units before applying the base device scale.
For curves, the initial line test uses the exact second derivative. Otherwise,
rounded midpoint distance selects a uniform subdivision depth using
`max(dx,dy) + min(dx,dy)/2`. Implied on-curve midpoints truncate to 26.6, whereas
subdivision midpoints round with positive ties in device Y-up coordinates.
A tolerance-only floating-point flattener misses these distinctions.

Coverage uses directed center ties. Epsilon-shifting the scan position can move
an interpolated crossing off an exact vertex. Dropout suppression uses complete
monotonic chains, rather than individual subdivided segments. A perpendicular
pass restores connected thin strokes while suppressing isolated tips. A glyph
with positive height but no sampled center row still receives one row.

All 35 unchanged controlled-Heros canvases in
[external-fonts-zd621-v1](../external-fonts-zd621-v1/README.md) now match exactly.
In the older 159-page contour/reconstruction corpus, mismatched pixels decrease
from 10,912 to 4,652, with no page's total increasing; exact pages increase from
20 to 66. All previously exact scaling/state canvases remain exact. Every changed
baseline retains explicit underpaint, overpaint and union counts; no broad
tolerance replaces the previous assertions. Residuals in constructed fonts and
reconstructions remain, so these results do not establish universal font parity.
