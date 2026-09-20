# Escaped field-block backslashes

Four unmodified ZD621 203 DPI HTTP preview responses, firmware V93.21.33Z,
captured 2026-09-20. Every request completed; requests were serialized with
five-second pacing. PW832 avoids preview-width resizing.

The 80 width controls cover fonts 0 and A and automatic wrapping. Another
16 controls cover repeated backslashes, escaped backslashes adjacent to
newline escapes, leading/trailing backslashes, and ordinary explicit newlines.
All 96 fields and all four full canvases are pixel-exact, with no alignment or
cropping. Source, response, and local pixel hashes are pinned with zero
underpaint and overpaint.

[Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB p. 187 specifies double-backslash escaping and requires CI13 (Item 1).
The printer also prints backslashes under CI27; the independently selectable
`block_backslash_without_ci13` compatibility option enables this departure.
The specification profile rejects the escape outside CI13. ASCII CI13 is now
supported; CI0/28 native cent glyphs are covered by legacy-backslash-zd621-v1. Soft-hyphen escape
markers are covered separately by field-block-markers-zd621-v1.
