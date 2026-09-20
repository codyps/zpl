# Field concatenation previews

26 complete, unmodified HTTP previews from a ZD621, 203 DPI, V93.21.33Z,
captured 2026-09-20 with serialized requests and five-second pacing. All
requests completed. Every source has PW832; images were not cropped, padded,
resized or registered. All 26 frames match every pixel, including Code 128,
EAN-13 and Unicode text. Source, native PNG and rendered-pixel hashes and
zero underpaint/overpaint counts are pinned in manifest.tsv.

The original five field-concat cases were recaptured unchanged. Independent
controls cover whole values, forward and backward substrings, range clamping,
missing and later bindings, duplicate bindings, nested concatenation, literal
markers in inserted data, FH decoding, Unicode character indexing, command
adjacency, FV, field/label scope, default/custom/space/comma delimiters, doubled
and unmatched markers, uppercase directions and negative lengths. The repeated
whole-value control and reset frames are pixel-identical to their originals.

Three compatibility options select measured departures from the documented
parameter forms and examples:

- concatenation_backward_reads_forward counts the starting position from the
  end, then reads forward. SPECIFICATION instead takes the requested characters
  ending there; the guide's b,1,4 example returns Data.
- concatenation_retains_delimiter preserves FE across intervening commands
  within the field. SPECIFICATION requires adjacency to field data. Neither
  retains FE after FS or after consuming the field data.
- concatenation_printer_syntax accepts uppercase directions, treats negative
  lengths as extending to the end, unescapes doubled markers, and uses # when
  the delimiter operand is a space or the current command delimiter.

All three are false in SPECIFICATION and true in ZD621_203_DPI. Missing bindings
contribute no data; references use the first supplied binding, so later explicit
bindings retain their own drawings without changing FE's earlier binding.
A binding defined later is unavailable to an earlier concatenation. Expanded
binding values can be referenced subsequently, but inserted markers are not
recursively interpreted. FN/FE values are local to a label. FH decoding happens
before FE expansion, whose result retains the 4096-byte field limit and the
original command's error offset. Non-ASCII FE delimiter operands remain
explicitly unsupported; Unicode field data and substring indices are covered.

Reference: Zebra ZPL II Programming Guide, ^FE pp. 191–192 and ^FN p. 200:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Run cargo test -p zpl --test field_concatenation --test field_concatenation_preview.
