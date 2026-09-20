# Retail field-data normalization

Twelve unmodified ZD621 203-DPI V93.21.33Z preview frames, captured 2026-09-20
in three serialized batches with five-second pacing. Every request completed;
each final normal-text reset matches the initial control pixel-for-pixel.
All sources use PW832. No reference is registered, padded, cropped or resized.

All twelve complete canvases match exactly, including barcode modules,
interpretation text and validation labels. The manifest pins source, PNG and
rendered-pixel hashes and zero underpaint/overpaint. The three matrices cover
EAN-8, EAN-13 and UPC-A with short/full/overlong data, supplied check digits,
spaces, alphabetic bytes and empty data. A 1–20-character length sweep exposes
truncation boundaries. Separate inputs and paired CVN/CVY fields independently
check normalization and validation order.

The specification profile now pads short input with zeros and truncates on the
left to the documented 7/12/11 data digits. Three options select measured
printer departures; all are false in SPECIFICATION and true in ZD621_203_DPI:

- `retail_non_digits_as_zero`: replace each nondigit byte with zero.
- `retail_ignore_supplied_check_digit`: discard a supplied final check digit
  at the full symbol length and recompute it.
- `retail_printer_overlong_data`: retain EAN-13's first digit with its last eleven
  data digits for input longer than thirteen bytes. For EAN-8, 9–12-byte inputs
  drop five leading characters; thirteen bytes first discard the final check
  digit. Longer EAN-8 inputs retain their final seven characters.

CVY validates the original data before ordinary normalization. Its short,
character and check errors remain S/C/E. EAN-8 overflow uses the EAN-13
validation width; overlong EAN fields retain the existing printer INVALID-S
choice. UPC-A instead reports INVALID-L, correcting an overbroad application
of `validation_retail_long_is_short`. UPC-E and DataBar aliases keep their own
input rules.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
B8 p. 83, BE p. 109, BU p. 142, and CV p. 167.
