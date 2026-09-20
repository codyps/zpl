# ZD621 Code 11 element widths

Five unmodified HTTP preview frames from ZD621 203 DPI, V93.21.33Z,
2026-09-19, cover 148 symbols. Three atlases vary module width 1–10 at
ratios 2.0/2.5/3.0; a holdout uses widths 3/7 with ratios 2.1/2.3/2.7/2.9;
a final atlas covers every module width 1–10 and every ratio tenth 2.0–3.0.
The payloads include 0, 9 and dash, which use extra-wide elements, and both
single/two-check configurations. PW832 avoids preview-width adjustment.

`code11_printer_element_widths` selects the captured quantization: ordinary
wide elements floor(module × ratio), extra-wide elements floor(module × ratio
× 5/3), calculated independently before accumulating positions. The prior
2W-X formula only agreed for some widths/ratios. Arithmetic in ratio tenths
preserves exact boundaries, including width 3/ratio 2.8 (14 extra-wide dots).
SPECIFICATION disables this override and retains the nominal geometry.

The manifest pins source and capture hashes, zero underpaint/overpaint, and
complete local pixel hashes. No alignment, cropping or reference resampling.
See `code11_widths_preview.rs`.

Sources: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^B1 p. 66 and ^BY p. 148. Exact element quantization is measured from these
printer previews rather than inferred from the illustrative bar pattern.
