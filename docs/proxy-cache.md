# Printer proxy cache

The printer-backed `/api/zpl-zd621` endpoint first enforces the
[rendering-only admission policy](proxy-validation.md), then stores every accepted ZPL submission
and each completed outcome in SQLite. This is separate from the GitHub Pages
preview, which remains browser-local and uploads nothing.

Set `DATABASE_URL` and run `diesel migration run` from `zpl-proxy-api/` before
starting the proxy with a fresh database. The standalone proxy does not
initialize the schema automatically; the [NixOS module](nixos.md) does so before
starting its managed service. The schema is defined in one initial migration;
there is no upgrade path for development databases from earlier revisions.

## Stored data and cache behavior

- `inputs`: exact UTF-8 ZPL bytes, SHA-256 deduplicated.
- `pngs`: original PNG bytes returned by the printer, SHA-256 deduplicated.
  These bytes are never annotated or re-encoded in the database.
- `render_cache`: successful input-to-PNG mappings, scoped by a digest of the
  configured printer URL, SGD endpoint, headers, cache namespace, and transport version.
  Printer identity and available rendering settings are stored separately as
  JSON in `printer_identity`.
  Header values and the printer URL are not themselves stored in this table.
- `png_requests`: one row per request, including cache hits, input, renderer key,
  timestamps, resulting original PNG, printer identity JSON, or a render error.
  A null completion timestamp means
  the process stopped or persistence failed before completion. Cache mappings
  require printer identity; pending requests and failures before preview can
  have no identity.

Inputs and request rows are committed before printer access. Successful responses
are returned only after the PNG and request outcome commit. Database errors fail
closed rather than silently bypassing persistence. Printer errors are recorded
but never reused as cached responses; the next request retries. Client disconnects
do not cancel an already-started render/persistence task.

The proxy serializes renders because the printer HTTP API uses a shared preview
object. Database work runs off the async executor. Run only one proxy instance per
printer: in-process serialization cannot protect against another proxy or direct
printer client overwriting the same preview object.

Responses carry `X-ZPL-Cache: hit` or `miss`. Add `"refresh": true` to JSON, or
`refresh=true` to URL-encoded or multipart form data, to force a new render. A
failed refresh invalidates the cached mapping, while previous PNG/history rows
remain. Change `--cache-namespace` after firmware/font/media/configuration changes.
Identical ZPL can render differently on a stateful printer; this cache does not
infer printer state, and cache hits do not execute the submitted commands.

## Printer identity and PNG metadata

On cache misses and explicit refreshes, the proxy reads identity and a fixed list
of rendering settings with SGD `getvar` commands over raw TCP, immediately before
requesting the HTTP preview and inside the same render lock. These are internal,
read-only queries; no submitted ZPL is sent to the SGD socket. The public label
admission policy still rejects queries and other non-rendering commands.

The SGD endpoint defaults to the host in `--zd621-url`, port **9100**, independently
of the HTTP URL's port. Use `--zd621-sgd-address IP:PORT` (IPv6: `[ADDRESS]:PORT`)
to override it. Both endpoints must reach the same physical printer; HTTP proxy
headers, credentials, and routing are not applied to the raw TCP connection.
A DNS lookup is bounded to five seconds; connection setup and all queries share
a fifteen-second deadline. Blocking SGD I/O runs off the async executor.

Required identity queries are `device.product_name` (model), `device.unique_id`
(serial), and `appl.name` (firmware). Optional settings include
`head.resolution.in_dpi`, `appl.link_os_version`, and `appl.bootblock`.
An explicit unsupported (`"?"`) or empty reply omits an optional setting. Missing
required identity, malformed responses, disconnects, and timeouts record a
retryable failure before the HTTP preview is requested. Each reply is bounded to
4096 bytes. There is no HTML-scraping fallback or speculative part-number field.
The shared `zebra-sgd` crate also supplies the firmware CLI's identity queries.
See the [Zebra SGD reference](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-sgd-printer-commands.html).

Successful HTTP responses include these headers and standard PNG `tEXt` chunks:

| HTTP header | PNG text keyword |
| --- | --- |
| `X-ZPL-Printer-Model` | `ZPL Printer Model` |
| `X-ZPL-Printer-Firmware` | `ZPL Printer Firmware` |
| `X-ZPL-Printer-Serial` | `ZPL Printer Serial` |

Two UTF-8 PNG `iTXt` chunks also accompany every returned image:

- `ZPL Source`: the exact submitted ZPL, including Unicode, whitespace, and line
  endings. This is the same source saved in `inputs.data` and sent to the printer.
- `ZPL Printer Configuration`: JSON with `schema_version: 1`,
  `source: "sgd"`, `capture: "before_preview"`, and a `settings` object.
  This captures reported print width, label length, resolution, media/print modes,
  darkness/speed, orientation, syntax prefixes, and related rendering settings.
  Keys are the exact SGD names, for example `head.resolution.in_dpi: "203"`.
  Values retain the printer's units and formatting; absent rows remain absent.

The configuration is a snapshot before rendering; the embedded ZPL can override
these settings. It is not a complete reproducibility bundle: these queries do not
expose every state variable, resident font mapping, or stored resource. Only the
explicit rendering-related settings in `zebra-sgd/src/lib.rs` are requested;
network settings, credentials, counters, and unknown rows are excluded. Each
setting is queried sequentially on the same TCP connection.
ZPL and configuration contents are embedded in the PNG and kept in SQLite, never
added to HTTP headers or telemetry. Anyone receiving the PNG can read its ZPL.

Metadata is added to a response copy only; the response is sent after the original
bytes, source, identity, and configuration have been committed. Even if PNG
annotation fails, the original
response bytes are stored with the error, and the failed mapping is invalidated.
Existing image data and unrelated PNG chunks are preserved.
The original image remains available in `pngs.data` for future metadata changes.
Identity is associated with requests and cache mappings, rather than deduplicated
PNG rows: different firmware versions can produce identical original images.
Cache hits use the exact matching source and saved identity/configuration, with
no printer access; after a firmware change they still describe the printer that produced that cached image. Refresh
or change the namespace to obtain a new rendering and identity. External printer
changes during a render cannot be prevented by the proxy's in-process lock.

## Validation

`cargo test -p zpl-proxy-api` covers request formats, database persistence,
renderer isolation, repeated requests, refresh/failure behavior, migration
round trips, UTF-8 source/configuration metadata, and mock HTTP/SGD printer cache
hits. `cargo test -p zebra-sgd -p zebra-firmware` covers bounded TCP query framing
and firmware identity queries. No real printer is needed.
