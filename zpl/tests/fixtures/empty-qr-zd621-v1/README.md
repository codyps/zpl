# Unstable native empty-QR previews

These seven unmodified ZD621 203-DPI previews (V93.21.33Z) are diagnostic
evidence, **not deterministic rendering targets**. All requests completed
successfully on their first HTTP submission, with `Connection: close` and at
least five seconds between requests. Source, PNG and raster hashes are pinned.

`empty-isolated`, `isolated-repeat1` and `isolated-repeat2` contain identical
ZPL bytes: BQ followed by FS without FD. Their printer pixels differ. The
QR payloads decode respectively to these byte strings (hex):

- `1a36383335210101012e504e47`
- `1a36603235210101012e504e47`
- `1a36a83235210101012e504e47`

`empty-prefix` and `prefix-repeat1` likewise submit identical `FDQA,` with no
payload and receive differing symbols. `after-code` draws Code 128 `1`, then
an empty QR field; the QR decodes to `__TMP758.PNG`, which is absent from the
submitted source. `empty-explicit` uses an empty FD and also produces stray
bytes. Decoding used the independent zxing-cpp decoder; decoded QR bytes are
recorded in the manifest, separately from the raw capture hashes.

The changing bytes and temporary filename are evidence of a native defect,
consistent with reading stale buffer contents. Its internal cause is not
proven. A deterministic renderer cannot reproduce this as a function of ZPL
alone. No compatibility option fabricates these bytes. The renderer leaves
fields without FD empty and rejects QR fields with missing data. Tests preserve
both the observed native instability and the renderer's deterministic behavior;
these cases are not counted as successful barcode accuracy comparisons.

References: [Zebra Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^BQ pp. 129–134 (field-data switches and barcode payload), ^FD p. 190 and
^FS p. 204. The guide does not specify embedding unrelated temporary filenames.
