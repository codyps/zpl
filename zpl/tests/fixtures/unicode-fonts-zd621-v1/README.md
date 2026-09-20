# Unicode font and advanced-default controls

40 unmodified HTTP previews from the ZD621, 203 DPI, firmware V93.21.33Z,
captured 2026-09-20 with serialized requests and five-second pacing.
All requests completed. Repeated first pages from each of six font captures
were pixel-identical, as were final reset controls. Every width is aligned
to 64 dots; references are not cropped, padded, resized or registered.

The font captures cover 95 ASCII glyphs at 40x24, all 27 Hebrew letters and
final forms at 40x24 and 32x0, the 40x24 native legacy backslash, and é/cent/
soft hyphen/eth at 40x24. A final capture measures native blank advances for
U+0378 and Arabic letters U+0627, U+0628, U+062D, U+0631 and U+0645 under PA0;
ASCII A anchors its independent verification string. Each capture includes
an independently composed verification string that matched every pixel.
The new Unicode sampler sends UTF-8 byte escapes under CI28 and resets PA
features to zero to measure glyphs without bidirectional reordering.

Two independent mixed ASCII/Hebrew/number holdouts cover all four rotations
at each size. Their separate text regions have at least 99.7956% foreground
IoU. These two frames retain 11 underpaint and 3 overpaint pixels altogether;
the other 38 frames are exact. The original advanced-text-0000 input from
zpl-comparison was recaptured unchanged and is now pixel-exact.

manifest.tsv pins source, raw PNG, local pixel hashes and directional paint
counts; assets.tsv pins all six exported font strikes. The test requires
exactness for sampling/verification/repeated/original-case frames and tests
foreground IoU separately for all eight rotated holdout fields. Enabled PA
features and unmeasured glyphs remain unsupported rather than silently ignored.

References: Zebra ZPL II Programming Guide, CI pp. 156–159, FH p. 190,
PA p. 315; https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Run cargo test -p zpl --test unicode_fonts --test unicode_fonts_preview.
