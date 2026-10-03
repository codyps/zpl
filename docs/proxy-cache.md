# Printer proxy cache and recovery

The proxy exclusively owns configured printers. Public clients select a configured
`name`; they cannot submit a printer address, management command, or reset request.
`GET /api/printers` returns the names only. Submit JSON, URL-encoded forms, or
multipart forms to `POST /api/printers/{name}/preview`, with `zpl` and optional
`refresh`. The bundled page lists the same names.

## Configuration

Run from `zpl-proxy-api/` with `DATABASE_URL` set and start with
`--printers /path/to/printers.json`. The executable embeds its Diesel migrations
and applies pending migrations automatically before serving requests. Applied
versions are tracked in Diesel's migration table, so restarts do not reapply them.
A migration failure stops startup; no separate CLI call or migration files are
needed at runtime. The database parent directory must already exist.

Example printer configuration:

```json
[
  {
    "name": "ZD621",
    "url": "http://printer-one.local/",
    "control_address": "printer-one.local:9100",
    "width": 832,
    "height": 1218,
    "serial": "EXPECTED-SERIAL"
  },
  {
    "name": "ZD621-V93",
    "url": "http://printer-two.local/",
    "control_address": "printer-two.local:9100",
    "width": 832,
    "height": 1218
  }
]
```

Names contain 1–80 ASCII letters, digits, dots, underscores or hyphens and are
case-sensitive. Width and height are explicit default canvas sizes in native dots;
use the printer's actual supported width. A label can override them with `PW` and
`LL`. `headers` optionally supplies HTTP headers; protect configuration files
containing credentials. `serial` optionally pins the expected physical printer.
The HTTP and SGD endpoints **must address the same physical device**. Duplicate
names/endpoints are rejected, and discovered serial numbers cannot be owned by two
names in one process, including DNS aliases. Run only one proxy instance per
physical printer and prevent other clients from accessing its management ports.

The named-printer configuration is required, including for a single printer.

## State, provenance, and cache

The [admission policy](proxy-validation.md) runs before database or printer access,
including cache hits and refresh. Accepted input/request rows are persisted before
identity lookup. SGD `device.unique_id` and `appl.name` are read before cache lookup;
if identity is unavailable, the request fails closed without issuing a preview.
For a previously observed printer, autonomous recovery uses its last-known identity
(including after proxy restart), with the failed lookup marked `identity-last-known`.
No label preview/cache hit proceeds until a fresh identity is available. If the
printer has never been identified, serial/firmware remain unknown, never invented
from a model name.

Each actual preview receives an explicit state prefix inside its `XA`/`XZ` format:
bitmap clearing, canvas, origins/shifts, orientation/mirroring/reversal, default
font, barcode defaults, encoding and field direction. Field-local block settings
are terminated by the required field separators. User field
bytes are preserved. This is a rendering-state reset, not a factory/network reset.
Trusted resident-font mappings and standard ZPL syntax remain provisioning
requirements. Each printer has its own serialization lock and eight admitted
request slots. Full queues return 503 without growing an unbounded task backlog.
The HTTP transport disables redirects and environment proxies.

SQLite stores:

- `inputs` and `pngs`: SHA-256-deduplicated original input and validated PNG bytes.
- `png_requests`: accepted request outcomes, including cache hits. A null completion
  timestamp means work/persistence was interrupted.
- `printer_requests`: public name and the observed serial/firmware snapshot for each
  request. Old rows have no fabricated provenance; identity failures have null
  serial/firmware. Cached results retain the identity of their renderer key.
- `preview_attempts`: every actual label, retry, control, and restart, with name,
  serial, firmware, phase, start/completion times, error and `failure_kind`.
  Preview timeouts are explicitly recorded as **`hang`**. Other categories are
  `rejected`, `unavailable`, and `recovery_failed`.
- `render_cache`: successful mappings scoped by public name, printer URL/headers,
  serial, firmware, configured dimensions, namespace, and reset/transport version.
- `permanent_errors`: confirmed repeated label failures under that same scope.
- `printer_recovery`: durable cooldown reservations to prevent restart storms.

Configuration/header secrets are hashed into keys, not copied to history or
telemetry. PNG bytes can be deduplicated across printers; the request and attempt
records preserve separate provenance. Database operations run in blocking tasks.
Successful responses wait for persistence. Started printer tasks continue if the
HTTP client disconnects.

`X-ZPL-Cache: hit|miss` reports successful or permanently rejected cache outcomes.
Public `refresh=true` forces a fresh successful render but cannot bypass a
quarantined label. Failed refreshes invalidate positive cache mappings. Firmware
changes automatically select a new cache scope; change `--cache-namespace` after
other trusted font/configuration changes or to intentionally retest a quarantined
label. Namespace changes are operator actions, not public request parameters.

## Automatic recovery

No user approval, button press, or refresh is needed to recover a preview hang.
A preview timeout is recorded as `hang`, then the proxy sends SGD
`! U1 setvar "device.reset" ""` once over the configured control endpoint.
After a settling delay it polls identity and a known-good control preview, with a
120-second recovery deadline. The control must decode to the exact expected
8×8 black square on the configured white canvas. Blank, malformed or stale
nonmatching controls are not evidence that preview works.

Once healthy, the proxy retries the label once with a fresh state prefix. If the
same label hangs again, recovery waits for the cooldown and restores the printer
before the label is quarantined. The original request receives a retryable 503
while that background recovery is pending. Repeated completed responses without an image
can also be quarantined if controls succeed. Matching label failures with healthy
controls return 422 and are durably cached; network failures, HTTP errors,
authentication failures and malformed PNGs remain retryable, never permanent
label verdicts. A failed control leaves the label retryable.

Restart cycles have a five-minute durable cooldown. If recovery fails or the
cooldown prevents an immediate restart, a single background task continues
recovery after cooldowns, without another user request. It checks for a healthy
control preview before restarting, avoiding a restart after spontaneous recovery.
Recovery has no finite retry limit: these are exclusively owned preview devices.
All restarts, including repeated label hangs, obey the same five-minute limit. Attempts use the original request ID for audit history even after that
request has received a retryable 503. Other printers continue normally. A serial/firmware change stops
an old identity's background recovery. In-process background work ends when the
proxy exits; interrupted records remain identifiable.

The reset command is documented in the [Zebra programming guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
`device.reset` p.755. It is a soft restart, not a remote power switch; network or
hardware failures can prevent it from succeeding.

## Internet deployment and validation

Keep printers on a private management network reachable only by the proxy. Place
TLS, request-rate limits, admission/access policy and connection/body deadlines at
the public reverse proxy. Allow long enough upstream response time for automatic
recovery. The application supplies strict ZPL admission, bounded per-printer work,
restart cooldowns, fixed configured targets and permanent-error quarantine; it
has no built-in authentication or disk-retention quota. Accepted labels and
results are retained, so size/monitor the database and protect its contents.
There is no public history, serial/firmware, management, or restart endpoint.

Run `cargo test --locked -p zpl-proxy-api -p zebra-http-api`. Loopback tests cover
identity snapshots, firmware cache separation, independent printers, reset
prefixes, hangs/restarts/retries, autonomous background recovery, quarantine,
transient failures, cache persistence, admission and HTTP formats. These are
simulator checks. [Live validation](proxy-live-validation.md) separately records
ZD621 and ZQ610 Plus state isolation and an observed ZQ610 Plus hang/recovery. NixOS confinement
has a separate Linux VM test; see [hosting](nixos.md).
