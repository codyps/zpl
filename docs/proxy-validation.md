# Printer preview admission policy

The proxy validates every submitted label before cache lookup, persistence, or
printer access. Malformed, unknown, or disallowed ZPL returns HTTP `400` with a
byte offset and a fixed diagnostic; the response does not echo submitted content.
The same rule applies to JSON, URL-encoded forms, multipart forms, cache hits,
and `refresh` requests. Previously cached forbidden labels are not served.
Rejected requests do not create input or render-history rows.

The [command-stream parser](parser-coverage.md) is lossless framing, not a
security validator: it preserves unknown commands and raw parameters. The proxy
therefore applies a separate positive command list and parameter grammar. It
requires exactly one complete `^XA` / `^XZ` label, standard `^`, `~`, `,` syntax,
and `^FS` after field data or a drawing. Settings must be inside the label.
There is no switch to bypass this policy.

## Accepted subset

| Purpose | Commands / restrictions |
| --- | --- |
| Label and fields | `XA`, `XZ`, `FS`, `FX` single-line comments |
| Layout | `FO`, `FT`, `LH`, `PW`, `LL`, `LS`, `LT`, `FW`, `PO`, `PM`, `LR`, `FR`, `FB`, `FP` |
| Text | `A0`, `AA`–`AH`, `CF` with resident IDs `0`, `A`–`H`; `FD`, `FV`, `FH`, `GS` |
| Encoding | `CI0`, `CI27`, `CI28`, without character remapping |
| Shapes | `GB`, `GC`, `GD`, `GE` |
| Barcodes | `BY`, `B1`, `B2`, `B3`, `B7`, `B8`, `B9`, `BA`, `BC`, `BE`, `BQ`, `BU`, `BX` |
| Inline graphics | `GFA`: hexadecimal/RLE and B64/Z64; decoded length, row dimensions, CRC and compression checks |

Command parameter counts, numeric syntax/ranges, and supported enum values are
checked. Omitted operands use printer defaults. This intentionally accepts a
subset of valid printer ZPL; it does not claim to emulate every firmware rule or
validate every barcode payload's meaning. For example, `BX` supports only `_`
as its explicit escape character. Unsupported commands/modes fail closed.

Fields must not contain raw control characters, including line breaks. Use
`^FB`'s `\&` for block line breaks or valid `^FH` escapes for field bytes.
Hex escapes are checked but never reparsed as executable ZPL. A literal caret
or tilde encoded with `^FH` remains printable field content, as specified in the
Zebra guide. Comments may have trailing formatting whitespace, but not embedded
control lines. Inter-command formatting whitespace is accepted.

Limits: 1 MiB submitted label, 25,000 decoded bytes per graphic, and 250,000 total
decoded graphic bytes per label. The existing bounded local graphic decoder
validates each isolated `GFA`; the proxy does not use the local renderer's whole
command set as authorization.

## Rejected operations

Everything outside the positive list is rejected, including:

- Host queries, status/configuration reports, directory listings (`H*`, `WD`,
  `~HS`, `~HQ`, `~WC`), and RFID read/write commands.
- Stored formats, images, and named fonts (`XF`, `XG`, `IL`, `IM`, `A@`),
  including commands that could render existing printer contents into a preview.
- Downloads, writes, deletes, font mappings/links, and other object operations
  (`DF`, `DG`, `DY`, `ID`, `IS`, `CW`, `FL`, `LF`).
- Printer/network configuration, resets, calibration, print/feed/pause commands,
  quantities, persistent bitmap templates (`MC`), clock/serialized/stored fields.
- All `~` control commands, syntax changes (`CC`, `CT`, `CD`), control-code
  alternatives, binary graphics (`GFB`/`GFC`), unknown commands, and mixed
  SGD/ZBI/other-language streams.

Binary graphics are excluded because firmware and framing can disagree on
invalid byte counts; the proxy must never authorize an opaque payload hiding
commands. Merely checking whether the local renderer accepts a stream is also
insufficient: it supports downloaded graphics and stored-image recall.

## Printer state prerequisite

Use a dedicated, trusted printer with default ZPL syntax, normal bitmap clearing
(`MCY`), and trusted resident-font mappings. Do not share it with clients that
change syntax, retain bitmap templates, substitute fonts, or execute arbitrary
commands. A validator cannot prove or undo pre-existing printer state. Even
built-in font identifiers can have been remapped by `CW`; a retained bitmap can
otherwise appear behind a later label. This policy prevents submitted requests
from making those changes; it is not a sandbox for a printer already configured
by an untrusted client. Change the cache namespace after trusted configuration
changes, as described in [proxy caching](proxy-cache.md).

The policy is based on the [Zebra programming guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
also bundled as `docs/zpl-zbi2-pg-en.pdf`: `FD`/`FH` pp. 190/193, `GF` p. 215,
`MC` p. 300, `WD` p. 361, `XF`/`XG` pp. 372–373, `A@` p. 62, `CW` p. 168,
and `CI` pp. 155–157.

Run `direnv exec . cargo test -p zpl-proxy-api --locked`. Regression tests exercise
malformed framing and operands, prohibited command families, syntax/binary/SGD
bypasses, valid text/barcodes/graphics, all HTTP input formats, refresh, and a
forbidden historical cache entry. Printer interactions use a loopback mock.
