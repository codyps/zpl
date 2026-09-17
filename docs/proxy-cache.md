# Printer proxy cache

The printer-backed `/api/zpl-zd621` endpoint first enforces the
[rendering-only admission policy](proxy-validation.md), then stores every accepted ZPL submission
and each completed outcome in SQLite. This is separate from the GitHub Pages
preview, which remains browser-local and uploads nothing.

Set `DATABASE_URL` and run `diesel migration run` from `zpl-proxy-api/` before
starting the proxy. Existing databases require the new persist-render-results
migration; startup fails clearly if it has not been applied. The standalone proxy
does not automatically migrate databases. The [NixOS module](nixos.md) runs
migrations before starting its managed service. Back up existing databases before
migration.

## Stored data and cache behavior

- `inputs`: exact UTF-8 ZPL bytes, SHA-256 deduplicated.
- `pngs`: exact successful preview response bytes, SHA-256 deduplicated.
- `render_cache`: successful input-to-PNG mappings, scoped by a digest of the
  configured printer URL, headers, cache namespace, and transport version.
  Header values and the printer URL are not themselves stored in this table.
- `png_requests`: one row per request, including cache hits, input, renderer key,
  timestamps, resulting PNG, or a render error. A null completion timestamp means
  the process stopped or persistence failed before completion. Old rows remain
  readable but their unscoped input/PNG mappings are not reused as cache hits.

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

## Validation

`cargo test -p zpl-proxy-api` covers request formats, database persistence,
renderer isolation, repeated requests, refresh/failure behavior, migration
round trips, and a mock-printer cache hit. No real printer is needed.
