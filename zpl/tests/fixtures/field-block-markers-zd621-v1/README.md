# Field-block soft-hyphen markers

Twenty-two unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z,
captured 2026-09-20. All requests completed and were serialized with five-second
pacing. PW832 avoids preview-width adjustment. Sources and PNGs are copied
unchanged; hashes, local pixel hashes and directional error counts are pinned.

The 520 fields cover:

- 160 width controls for fonts 0/A, alphanumeric soft markers and silent `\(`.
- 140 held-out fields with multiple markers, preceding/following words,
  leading markers, automatically split segments, and indentation 0/7.
- 108 initial syntax controls at three widths, CI0/27, punctuation and newline
  escapes. The raw source is authoritative: the initial generator's intended
  double-backslash control was actually a single marker, so genuine literal
  backslashes are covered by the separate backslash suites.
- 48 further punctuation-marker controls, including right parentheses, stars,
  slashes, underscores, exclamation/question marks, colons, and equals signs.
- 64 rotation/alignment controls across FO/FT, N/R/I/B and L/C/R/J.

Eighteen frames are pixel-exact. Each of the four rotated font-0 frames retains
6 underpaint and 8 overpaint dots. All 32 fields in those frames are checked
individually; minimum foreground IoU is 99.8949%. Their rectangles must cover
every reference or candidate ink pixel exactly once. Other frames must match
exactly; blank canvas cannot dilute the foreground metric.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 186–187 defines alphanumeric soft-marker escapes and preferred breaks.
Both profiles interpret those markers. `block_soft_hyphen_printer_layout`
selects the captured departures: nonalphanumeric markers, silent `\(`,
blank-row handling at narrow fits, retained width decisions across indentation,
and overflowing remainder lines. Such overflow can also consume following
words without wrapping. The specification profile uses ordinary width and
indentation rules. Existing options independently select AD hyphen metrics and
CI27's eth glyph. Literal U+00AD and escaped backslashes remain distinct.

The width and multiple-marker atlases establish inclusive explicit-marker fit,
which differs from the printer's strict automatic-hyphen fit. An overlong
segment can be split automatically; its remainder then joins the next segment
as a normal word, including a space if they share a line. These cases are
pinned against printer output, not a synthetic expected-line algorithm.
