# Printer preview admission policy

Each named `POST /api/printers/{name}/preview` endpoint selects its policy from
`admission` in the operator's printer configuration: `restricted` (the default)
or `unrestricted`. Clients cannot select or override the policy in a request.
Both policies accept JSON, URL-encoded forms and multipart forms containing a
UTF-8 `zpl` string. Axum's 2 MiB HTTP body limit applies to both.

## Unrestricted endpoints

Set `"admission": "unrestricted"` for a named printer. The proxy performs no ZPL
command, operand, syntax, framing, graphic or label-size validation. It sends the
submitted string unchanged to the configured printer's HTTP **preview** service.
This is not a raw TCP endpoint or a guarantee that firmware supports every input.

Both endpoint policies use the same caching and recovery lifecycle. Initialization
and recovery issue separate control previews with explicit rendering defaults.
User submissions are cached, retried after failures, and recovered with automatic
printer resets when needed, subject to the shared restart cooldown. Unrestricted
user input remains unchanged on each attempt; it need not contain a standard
`^XA` label. `refresh=true` forces a fresh render unless the input is quarantined.
Confirmed repeated failures are cached and return HTTP `422`; transient outages
return `503` and remain retryable. State-changing commands may therefore be
replayed on a retry, or skipped on a cache hit.

Fixed printer targets, live identity/serial checks, per-printer serialization,
eight request slots, transport timeouts, PNG validation and history remain in
force. There is no built-in authentication: access to an unrestricted endpoint
belongs in the deployment's access policy. Configure each physical printer once;
do not share a device across restricted and unrestricted names.

## Restricted endpoints

The restricted policy checks command effects and stream boundaries, leaving
ordinary rendering operand counts, numeric ranges and enum values to firmware.
It is a separate positive command list, independent of parser/local-renderer
support. It accepts rendering, stored image/font reads, and variable fields;
it excludes device operations, host queries, writes and executable stored formats.

Admission runs before cache lookup, persistence or printer access, including
cache hits and `refresh` requests. Rejections return HTTP `400` with a byte offset
and a fixed diagnostic, without echoing input or creating history rows.

Exactly one complete `^XA` / `^XZ` label is required, using standard `^`, `~`, `,`
syntax, with settings inside the label and `^FS` after field data or a drawing.
Unknown commands fail closed. Inter-command formatting whitespace is accepted.

| Purpose | Commands |
| --- | --- |
| Label and fields | `XA`, `XZ`, `FS`, `FX`, `FD`, `FV`, `FH`, `FC`, `FE`, `FN`, `SN`, `SF` |
| Layout | `FO`, `FT`, `FM`, `LH`, `PW`, `LL`, `LS`, `LT`, `FW`, `PO`, `PM`, `LR`, `FR`, `FB`, `FP`, `PA`, `TB` |
| Text | `A0`–`A9`, `AA`–`AZ`, `A@`, `CF`, `GS` |
| Encoding | `CI`, including rendering encodings and character remapping |
| Shapes | `GB`, `GC`, `GD`, `GE` |
| Barcodes | `BY`, `B0`–`B5`, `B7`–`B9`, `BA`–`BF`, `BI`–`BM`, `BO`–`BU`, `BX`, `BZ` |
| Stored images | `XG`, `IL`, `IM` |
| Inline graphics | `GFA`: hexadecimal/RLE and B64/Z64, with bounded decoding and dimension/CRC/compression validation |

Admission does not imply that a particular firmware accepts an operand or supports
a command. Ordinary operands must contain no embedded control characters or
command prefixes. This includes prefix characters used as a `BX` escape operand;
other printable escape characters are accepted.

Fields cannot contain raw control characters, including line breaks. Use `^FB`'s
`\&` or valid `^FH` escapes instead. Hex escapes are checked but never reparsed
as executable ZPL. Comments may have trailing formatting whitespace but no
embedded control lines. Boundary commands and `FH` retain their structural checks.

Resource limits remain: 1 MiB submitted label, 25,000 decoded bytes per graphic,
and 250,000 total decoded graphic bytes per label. The isolated `GFA` decoder
checks graphics without authorizing the local renderer's entire command set.

All accepted labels participate in caching and bounded recovery, including stored
image/font references and clock/variable/serialization fields. Use `refresh=true`
for fresh output after the underlying content changes; change the cache namespace
to invalidate a quarantined result. Cache identity includes admission policy and
the reset implementation version.

## Operations requiring an unrestricted endpoint

- Host queries, reports and directory listings (`H*`, `WD`, `~HS`, `~HQ`, `~WC`),
  and RFID operations.
- Stored-format recall (`XF`): a stored format can execute commands; unlike an
  image or font reference, it cannot be admitted as a simple content read.
- Downloads, writes, deletes, font mappings/links and object operations
  (`DF`, `DG`, `DY`, `ID`, `IS`, `CW`, `FL`, `LF`).
- Printer/network configuration, resets, calibration, print/feed/pause commands,
  quantities and persistent bitmap templates (`MC`).
- All `~` commands, syntax changes (`CC`, `CT`, `CD`), control-code alternatives,
  binary graphics (`GFB`/`GFC`), unknown commands and mixed SGD/ZBI streams.

## Printer state

Restricted endpoints expect trusted printer provisioning, standard ZPL syntax,
normal bitmap clearing and trusted stored resources/font mappings. Stored image
and named-font reads intentionally expose those resources to preview clients.
The proxy injects rendering defaults and bitmap clearing, and verifies a control
preview on first use and during recovery. It cannot undo arbitrary font mappings,
changed syntax or every firmware-specific rendering state. Reprovision a device
before switching it from unrestricted use to restricted use, and change the cache
namespace after trusted configuration changes that affect output.

The policy references the [Zebra programming guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
bundled as `docs/zpl-zbi2-pg-en.pdf`: `A`/`A@` pp.60–62; barcodes pp.64–150;
`CI` pp.155–159; fields pp.186–209; `GF` p.215; `IL`/`IM` pp.247–248;
`PA` p.315; `SF`/`SN` pp.335/341; `TB` p.356; `XF`/`XG` pp.372–373.

Run `cargo test --locked -p zpl-proxy-api -p zebra-http-api`. Loopback tests cover
restricted boundaries, expanded rendering commands, configured policy selection,
verbatim unrestricted forwarding, caching/refresh, retries and reset recovery in
both modes, persistence and HTTP input formats. No
physical printer is needed for these tests.
