# Resident cent glyphs

Thirty-two unmodified HTTP preview frames from ZD621 203 DPI firmware
V93.21.33Z, captured 2026-09-20 with serialized requests and five-second
pacing. Fifteen single-glyph extraction pages measure ink and advance via
sentinels; fifteen separate triple-cent pages independently verify composition.
Two held-out atlases cover 60 ordinary/centered-block fields using CI27 and
CI28 across all fifteen embedded font settings. Every full canvas is exact.
Extraction pages use widths divisible by 64; held-out atlases use PW832.

The supplements add U+00A2 at eight font-0 settings and resident A/B/D/E/F/G/H.
E/H are blank, advancing glyphs, as verified by both sentinels and held-out
ordinary text. C shares D. Sources, response hashes, pixel hashes, glyph
metrics, and packed asset hashes are pinned. No image registration is used.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^CI pp. 156–159 and font metrics Tables 29/31 pp. 1582–1583.
Capture uses the repository extract-font example with --characters ¢,
--verify-text ¢¢¢ and --delay 5. These encoded cent glyphs are distinct from
some bitmap fonts' native CI0 backslash replacement. The separate
legacy-backslash-zd621-v1 suite covers that mapping; the glyphs here do not
substitute for those native samples.
