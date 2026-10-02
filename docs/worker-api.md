# Renderer API on Cloudflare Workers

The first API runs the local Rust renderer as WebAssembly inside a Cloudflare
Worker. It accepts public requests, applies rate and resource limits, and returns
compressed PNGs through a subset of Labelary's HTTP interface. It needs no printer,
database, storage bucket, or rendering backend.

The Worker name is `zpl-render-api`. Select the target account through the
`CLOUDFLARE_ACCOUNT_ID` environment variable; account identifiers are not checked in.
The implementation and local runtime checks are complete. A public deployment
and its URL still require Cloudflare authentication and verification.

## Hosting decision

Workers is the recommended starting point: the renderer already supports Wasm,
the measured bundle and memory fit its limits, and PNG responses incur no Workers
egress charge. Cloud Run is the fallback if larger labels or future native
dependencies make the Worker memory limit impractical.

| Host | Deployment effort for this repository | Scaling and constraints | Cost considerations |
| --- | --- | --- | --- |
| Cloudflare Workers | Small JS HTTP adapter plus the Rust Wasm build; implemented here | Automatic distribution; 128 MiB per isolate, shared by concurrent requests and Wasm | Paid plan starts at $5/month; requests and CPU billed above allowances; no Worker egress charge |
| Google Cloud Run | Add a native HTTP server and container; reuse the renderer | Managed regional autoscaling and scale to zero; configurable memory and instance limits; cold starts need measurement | Can be near zero at low traffic; active CPU, memory, requests, and outbound data determine growth cost |
| AWS Lambda | Add a Rust Lambda adapter and deployment/IAM configuration | Managed regional scaling subject to quotas; tune memory and concurrency; cold starts need measurement | Generous compute allowance; requests, duration, outbound data, and any API Gateway/WAF layer add cost |

Platform references: [Workers limits](https://developers.cloudflare.com/workers/platform/limits/),
[Cloud Run autoscaling](https://docs.cloud.google.com/run/docs/about-instance-autoscaling),
[Lambda scaling](https://docs.aws.amazon.com/lambda/latest/dg/lambda-concurrency.html).
The deployment-effort assessment is specific to this repository; only Workers
has been implemented and tested.

### Monthly cost scenarios

These are planning estimates in USD using list prices checked on 2026-10-02,
with each account's included usage otherwise unused. They are not measured
cross-provider performance or complete invoices.

| Renders per month | Workers at 10 ms CPU | Workers at 50 ms CPU | Cloud Run compute and requests | Lambda compute and requests | PNG transfer at 50 KiB each |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100,000 | $5.00 | $5.00 | $0.00 | $0.00 | 4.8 GiB |
| 1 million | $5.00 | $5.40 | $0.00 | $0.00 | 47.7 GiB |
| 10 million | $6.40 | $14.40 | $23.23 | $3.47 | 476.8 GiB |
| 100 million | $51.40 | $131.40 | $286.48 | $96.47 | 4,768.4 GiB |

Workers includes 10 million requests and 30 million CPU milliseconds per month;
overages are $0.30/million requests and $0.02/million CPU milliseconds. The table
includes its $5 base fee. Rejected requests and health checks also consume usage.
[Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/).

Cloud Run assumes request-based billing in Iowa, one vCPU, 512 MiB, and **100 ms
of billable instance time per request**, without overlap or warm instances.
CPU costs $0.000024/vCPU-second, memory $0.0000025/GiB-second, and requests
$0.40/million after the listed free allowances. Concurrency can amortize instance
time; startup and slow responses can increase it.
[Cloud Run pricing](https://cloud.google.com/run/pricing).

Lambda assumes US East x86 on-demand, 512 MiB, and **100 ms billed duration**,
at $0.0000166667/GB-second and $0.20/million requests after its free allowances.
These duration assumptions must be benchmarked independently: equal memory does
not imply equal CPU allocation or rendering speed.
[Lambda pricing](https://aws.amazon.com/lambda/pricing/).

Cloud Run and Lambda columns exclude outbound transfer, API gateways, rate-limit
infrastructure, build/storage charges, and logging. Workers' no-egress pricing
is particularly useful for image responses. None of the estimates includes
taxes, support, or attacks. The public rate limits below are not a global spend
cap. At 100 million requests/month, revisit the initial service throttle and
measure actual regional bursts before raising it.

## HTTP contract

```text
POST /v1/printers/{6|8|12|24}dpmm/labels/{width}x{height}/{index}/
GET /health
```

Dimensions are inches; the label index starts at zero. A trailing slash is
optional. PNG is the default output and the only supported output format.
The implementation follows [Labelary's POST contract](https://labelary.com/service.html),
sections 1–3 and 6.6, within the scope below.

```sh
# Replace with the URL printed by Wrangler, or use the local dev URL.
api_origin=http://localhost:8787
curl --fail-with-body --silent --show-error \
  "$api_origin/v1/printers/8dpmm/labels/4x6/0/" \
  --header 'Accept: image/png' \
  --data-binary @docs/examples/local-label.zpl \
  --output /tmp/label.png

# Equivalent multipart upload:
curl --fail-with-body --silent --show-error \
  "$api_origin/v1/printers/8dpmm/labels/4x6/0/" \
  --form file=@docs/examples/local-label.zpl \
  --output /tmp/label.png
```

Raw bodies accept `application/x-www-form-urlencoded`, `application/octet-stream`,
or `text/plain`. Raw form content is ZPL bytes, without URL decoding: `+`, `%`,
`&`, NUL, and binary graphics remain intact. Multipart requests must contain
exactly one `file` field. ZPL is never placed in a URL by this API.

| Behavior | First version |
| --- | --- |
| Output viewport | URL dimensions, even when `^PW` or `^LL` changes layout |
| Density conversion | Integer DPI 152, 203, 304, 609; floor each inch dimension multiplied by that DPI |
| PNG physical resolution | 6,000 / 8,000 / 12,000 / 24,000 pixels per meter respectively |
| Multiple labels | Select one PNG by index; `X-Total-Count` reports rendered label count |
| Rendering profile | Explicit `SPECIFICATION`; see [renderer coverage](local-renderer.md) |
| Diagnostics | Text errors; `X-Render-Warnings` count and `X-Renderer-Version` on renderer responses |
| Browser access | Public CORS and `OPTIONS`; responses use `Cache-Control: no-store` |
| Other Labelary features | GET rendering, PDF, rotation, linter output, extraction, and format conversion are not implemented |

Density behavior was measured against the public Labelary endpoint on 2026-10-02
using synthetic `^XA^FO1,1^GB2,2,2^FS^XZ` input. Observed canvases included
`8dpmm/4x6` → 812×1218, `12dpmm/4x6` → 1216×1824,
`24dpmm/1.5x.5` → 913×304, and `8dpmm/1.234x2.345` → 250×476.
The tests pin these sizing rules. This is HTTP and sizing compatibility, not a
claim of pixel parity with Labelary or every printer. Unsupported ZPL returns an
error; supported approximate text can produce warnings. All labels are parsed
before selection, so an invalid unselected label also fails the request.

Status codes: `400` invalid parameters/ZPL or unsupported headers, `404` unknown
endpoint or missing label, `405` unsupported method, `406` unsupported output,
`408` body timeout, `413` resource limit, `415` unsupported input type, `429`
rate limit, and `503` unavailable limiter or busy isolate. Unexpected failures
return a generic `500`. `X-Rotation` and `X-Linter` are explicitly rejected.

## Public access and resource limits

No API key is required. The Worker checks Cloudflare rate-limit bindings before
reading the request body. Missing bindings fail closed.

| Limit | Initial setting |
| --- | --- |
| Client rate | 30 requests per 10 seconds per `CF-Connecting-IP` and Cloudflare location |
| Service rate | 600 requests per 60 seconds per Cloudflare location |
| Request body | 1 MiB ZPL; multipart envelope gets 64 KiB additional allowance |
| Body reading | 15 seconds; up to four admitted requests per isolate |
| Canvas | At most 15 inches and 4096 dots per side; at most 10,000,000 pixels |
| Labels | 50 per submission |
| Scene complexity | 100,000 total path segments; 100,000 stored graphic segments |
| CPU | 1000 ms per invocation on the deployed Paid plan |

Cloudflare's [rate-limit counters](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)
are approximate and local to each location. They do not provide strict global
quotas or billing limits. Shared NATs share a client allowance. `429` responses
include `Retry-After` (10 or 60 seconds); clients should back off with jitter.
The CPU cap is enforced by Cloudflare, not local workerd. A Free-plan deployment
is unsuitable because some measured labels exceed its 10 ms CPU allowance.

The API uses `render::render_with_limits` to set service-specific budgets,
retaining `render::Limits::DEFAULT` for the remaining fields. These service
settings are independent of the CLI, which renders without resource ceilings.
Graphic expansion is checked before growing paths; `^PW`, `^LL`, recalled
formats, and all labels remain subject to the limits. The first limit reached
wins, so complex batches may fail below 50 labels. Raster scan-work and other
existing renderer budgets still apply. The root `render` API keeps its defaults.

The service does not persist submitted ZPL, PNGs, or client addresses. The IP is
used transiently as Cloudflare's rate-limit key. There are no cache/database
bindings and no application request logs; Worker observability logs are disabled.
Cloudflare still processes requests and maintains its platform usage metrics.

## Build and test

Use Rust, Node.js 24, and the pinned wasm-bindgen CLI:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
npm ci --prefix zpl-render-api
cargo test --locked -p zpl --test render_limits
cargo test --locked -p zpl-render-api
bash scripts/build-worker.sh
cd zpl-render-api
npm test
npm run test:worker
npm run benchmark
npx wrangler deploy --dry-run --outdir .wrangler/build
npm run dev
```

HTTP tests cover byte preservation, negotiation, streaming limits, timeout,
concurrency, CORS, and fail-closed throttling. Miniflare loads the compiled Wasm
under workerd and checks real PNGs, binary multipart, density/canvas limits,
graphic expansion, recovery, and Cloudflare rate limiting. Native adapter tests
compare decoded pixels with the specification raster. The renderer regression
suite also covers printer captures and conformance fixtures.

Keep `wasm-bindgen` and its CLI pins aligned with the browser build. Generated
`pkg/`, `.wrangler/`, credentials, and `node_modules/` are ignored; Cargo and npm
lockfiles are checked in. Local HTTP tests and benchmarks do not call Labelary,
Cloudflare's control plane, or printers.

### Local sizing evidence

The 2026-10-02 Node 24.21.0 run of `npm run benchmark` used three warmups and ten
measured iterations per case. The Wasm module was 1,698,142 bytes. These are local
Wasm wall times, not Cloudflare CPU billing or production latency measurements.

| Fixture | Median | PNG size | Wasm memory high water |
| --- | ---: | ---: | ---: |
| Shipping label, 812×1218 | 3.12 ms | 10,130 bytes | 10.88 MiB |
| 48 text fields, 812×1218 | 10.78 ms | 27,812 bytes | 14.06 MiB |
| Shipping label, 2436×3654 | 19.57 ms | 57,020 bytes | 22.56 MiB |
| Maximum canvas, 4096×2441 | 20.84 ms | 53,841 bytes | 22.56 MiB |
| 50 simple labels, selecting one | 2.48 ms | 9,434 bytes | 22.56 MiB |
| 50 shipping labels, selecting one | 8.99 ms | 10,130 bytes | 22.56 MiB |
| Excessive rectangle batch / alternating graphic | 26.02 / 1.69 ms | Rejected with 413 | 22.56 MiB |

Memory is the cumulative Wasm linear-memory high-water mark across the suite,
excluding JavaScript, runtime overhead, and simultaneous uploads. It establishes
feasibility for these fixtures, not a worst-case bound for every valid input.
Measure deployed CPU percentiles, memory errors, and 429s before increasing limits.

## Deployment and verification

Use Workers Paid in the target account and set `CLOUDFLARE_ACCOUNT_ID` through
your shell environment or secret store. Authenticate securely with
`npx wrangler login` or provide `CLOUDFLARE_API_TOKEN` through the environment or
secret store. Keep account IDs and tokens out of the repository. A token scoped
to this account needs Workers Scripts edit access; use Cloudflare's Workers
deployment token template and restrict its account resources.

From `zpl-render-api/`:

```sh
npx wrangler whoami
npx wrangler deployments list
npm run deploy
```

Before the first deploy, confirm the Worker name does not belong to an unrelated
service, and that namespace IDs `9428751` and `9428752` are not already used by
other rate-limit policies in the account. Wrangler supplies the `workers.dev`
hostname; the account ID alone does not determine it. No custom domain is needed.

The [Renderer API workflow](../.github/workflows/worker.yml) builds and tests on
pushes and pull requests. Configure both `CLOUDFLARE_ACCOUNT_ID` and
`CLOUDFLARE_API_TOKEN` as repository secrets. A manual run on `main` with
`deploy=true` deploys the tested build. It does not deploy automatically on pushes.

Verify the printed public URL with `GET /health`, then run the two curl examples
above against it. Confirm PNG dimensions 812×1218, `Content-Type: image/png`,
`X-Total-Count: 1`, and `Cache-Control: no-store`. Check that `Accept:
application/pdf` returns 406 and an unavailable label index returns 404.
Local workerd success and Wrangler's dry run do not establish that the public
deployment works. Use `npx wrangler deployments list` and `npx wrangler rollback`
to inspect and restore a previous deployment when necessary.
