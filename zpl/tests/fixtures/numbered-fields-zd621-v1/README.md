# Inline numbered-field previews

19 complete raw HTTP previews from a ZD621, 203 DPI, V93.21.33Z, captured
2026-09-20 with serialized requests and five-second pacing. Every source has
PW832. No PNG was cropped, padded, resized or registered. All 19 frames match
with zero underpaint and zero overpaint, including barcode and text pixels.
The original numbered-fields-inline case was recaptured without modification.
The independent repeated forward-reference and reset controls are identical.

Controls cover forward references, rebinding, unbound/empty data, several
pending references, independent field numbers, mixed text/barcode styles,
FH decoding at the binding versus the reference, prompts, default number zero,
number 9999, FV data and state at the next label. All source, PNG and rendered
pixel hashes are pinned in manifest.tsv; tests use ZD621_203_DPI explicitly.

The preview differs from the shared-field behavior described by ^FN p. 200:
with earlier unresolved references, a binding populates those references and
does not draw at its own origin. Without earlier references, it draws normally.
A later data-less FN field remains blank. Multiple earlier references all get
the same next supplied value. The independent numbered_fields_forward_only
option selects this behavior. SPECIFICATION instead supplies the last assigned
value to all data-less fields of that number within the label, retaining each
binding field's own value. Each reference keeps its font, position, barcode,
and other layout settings; FH decoding belongs to the data's binding.

Stored DF/XF formats and serialization combined with FN are not implemented.
A subsequent custom-prefix printer request timed out and has no verified
image; it was not replayed and is excluded from this fixture set. The printer was
recovered using its Tasmota plug; recovery-control is a fresh, pixel-exact
preview after the automatic restart. Prefix and
control-character handling have separate offline regression tests.

Reference: Zebra ZPL II Programming Guide, ^FN p. 200:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Run cargo test -p zpl --test numbered_fields --test numbered_fields_preview.
