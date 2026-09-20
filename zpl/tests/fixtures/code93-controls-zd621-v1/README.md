# ZD621 Code 93 payload-control interpretation

Four unmodified native ZD621 203-DPI V93.21.33Z frames are pixel-exact,
including both bars and captions. `compact-code93-substitutes` is the original
failing comparison-corpus case, checked against its complete capture manifest.
`pairs` contains 37 independent X/pair/Z fields: all 26 dollar-shift controls,
five percent-shift controls, NUL, @, grave accent, and three DEL substitutes.
`holdouts` varies module widths 1–3, checksum interpretation, and all four
orientations. `combined-checks` adds eight payload-control cases with extended C/K checks.
In total the frames cover 55 barcode fields.

ZPL uses `&`, apostrophe, `(`, and `)` for Code 93 shift values. In the native
caption, decoded bytes 1–26 produce a solid cell followed by A–Z; bytes 27–31
produce a solid cell followed by `[`, the native cent image, `]`, `^`, or `_`.
NUL produces a solid cell followed by @, while DEL produces a solid cell and
space. The grave-accent substitute uses an apostrophe glyph. Normal printable
payloads retain their existing behavior. These are caption rules, not changes
to the encoded barcode data or its C/K checksums.

`code93_control_interpretation` independently selects this departure. It is
false in SPECIFICATION and true in ZD621_203_DPI. Existing options independently
control start/stop caption delimiters and extended C-check preview behavior.
The new tests verify that toggling payload-control interpretation leaves bars
unchanged. Existing exhaustive extended-C/K controls remain regression coverage.

Reference: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`^BA` pp. 87–89, especially the full-ASCII control-code substitute table.
That table defines encoded characters; the error-cell caption presentation is
measured printer behavior. JSON records each successful native submission and
the exact field list; the manifest pins sources, native PNGs, and local pixels.
No PNG was resized, aligned, or edited.
