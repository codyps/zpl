# Repository Guidelines

## Project Structure & Module Organization

This Rust 2021 Cargo workspace contains these crates:

- `zpl-font-extract/`: Resident-font sampling, bitmap extraction, export, and verification. Uses the bitmap strike types and decoder in `zpl::bitmap_font`.
- `raster-diff/`: Raster image decoding and comparison.
- `zpl-wasm/`: Browser rendering bindings.
- `zpl/`: ZPL parsing, command/format types, and rendering. Parser unit tests live in `src/parse/test.rs`; renderer unit tests are colocated with their modules; integration tests and printer captures live in `tests/`.
- `zebra-http-api/`: Zebra printer HTTP rendering client, with a `zebra-render` example.
- `zebra-firmware/`: Raw TCP firmware update CLI with printer identity checks and loopback simulator tests.
- `zpl-proxy-api/`: Axum proxy, SQLite render cache and request history, Diesel models and migrations, and browser assets in `assets/`.

Shared ZPL fixtures and optional TOML metadata live in `test-data/`. The `zpl-to-svg` example renders local PNG/SVG output. `site/` contains the browser-local Wasm preview; `nix/` contains proxy packaging, the NixOS module, and VM tests. Cross-library benchmarks live in the separate sibling `zpl-comparison` repository; keep this repository’s own regression tests here.

## Build, Test, and Development Commands

Run workspace commands from the repository root:

- `direnv exec .`: enter the development shell with Rust, Clippy, rustfmt, Diesel CLI, and SQLite; sets `ROOT_PATH` and `DATABASE_URL`. Uses the nix flake, but caches it.
- `cargo build --locked --workspace`: build all crates from the checked-in lockfile.
- `cargo test --locked --workspace`: run workspace tests.
- `cargo test --locked -p zpl --lib`: run parser and renderer unit tests.
- `cargo fmt --all -- --check`: check Rust formatting.
- `taplo fmt`: automatically format TOML files; `taplo fmt --check` checks formatting as in CI.
- `cargo clippy --locked --workspace --all-targets`: inspect lint diagnostics.
- `nix fmt`: format the Nix configuration.

To run the proxy, enter `direnv exec .`, create the database directory with `mkdir -p _db`, then:

```sh
cd zpl-proxy-api
diesel migration run
cargo run -- --zd621-url http://printer.local/ --bind-addr 127.0.0.1:3000
```

Replace the printer URL with your device address. Run from this crate directory because static assets use a relative path.
The proxy requires `DATABASE_URL` and current migrations. It stores submitted ZPL,
PNG results, errors, and request history; see `docs/proxy-cache.md`.

Preserve the flake's conditional `mbx` Cargo-shim precedence and separate unstable/Intel-Darwin Nixpkgs inputs. Check the resolved input tree when changing overrides. Keep CI job timeouts, bounded download retries, and cancellation of superseded builds; do not automatically retry failing tests to obtain a green result.

## Architecture & Implementation Contracts

- Keep `render` and `Options` as the root convenience exports; supporting APIs belong in modules. Parsing frames lossless byte streams, including unknown commands and binary data; it is not parameter validation or authorization. See [parser coverage](docs/parser-coverage.md).
- Rendering produces an output-independent scene; adapters consume scenes rather than interpreting ZPL. Keep rasterization under `output::raster`, with `rasterize()` wrapping `rasterize_into()`. Preserve ordered black/white/invert compositing and validation before destination mutation. See [local renderer](docs/local-renderer.md).
- Barcode encoders must be original implementations, each in its own module. Do not copy, adapt, or vendor another encoder to satisfy “no dependencies.” Independent decoder libraries belong in dev-dependencies only. Cite specifications and distinguish unsupported modes with explicit errors. See [barcode coverage](docs/barcodes.md).
- Keep sampling, fitting, extraction, and verification tooling in `zpl-font-extract`; retain only rendering-required bitmap types/data/decoding in `zpl`. Browser bindings belong in `zpl-wasm`. Avoid adding runtime dependencies to core rendering without a task-specific reason consistent with the user's original-implementation constraint.

## Dependency Updates

- Check current registry data before declaring dependencies current; a cached `cargo outdated` result can miss releases. Update direct requirements and the root `Cargo.lock`, including incompatible releases when the required migrations are practical. Use stable releases unless prereleases are requested.
- Review all dependency surfaces: Cargo manifests/lockfile, `site/package.json` and its lockfile, `flake.lock`, and pinned Actions/tools in `.github/workflows/`. Keep `wasm-bindgen` and its CLI version in `scripts/build-site.sh` and the Pages workflow aligned. Keep Actions pinned to commit SHAs with readable version comments.
- Respect upstream compatibility constraints. Check adapter dependencies before upgrading shared types such as OpenTelemetry; document any retained version and the blocking upstream requirement. Do not claim every package is latest when exceptions remain.
- Use the workspace lockfile; do not recreate crate-local Cargo lockfiles. Separate dependency upgrades from release preparation (`release-plz.toml` disables automatic dependency updates).
- Decoder upgrades can remove old limitations. Replace assertions expecting unsupported behavior with successful round-trip assertions when support arrives, and run the affected tests.

## Coding Style & Naming Conventions

Use rustfmt defaults, including four-space Rust indentation. Use `snake_case` for functions/modules and `UpperCamelCase` for types. Keep parsing, printer transport, and proxy concerns in their respective crates. Add database changes as timestamped Diesel migrations with `up.sql` and `down.sql`; keep `src/schema.rs` synchronized.

Ensure the standards, documents, specifications, etc used are referenced in comments in code and tests. Identify specific pages/paragraphs if possible. Include a link to the source if available online. Workspace crates use OSL-3.0 with the root `LICENSE` and per-crate packaging symlinks. Access to a private specification does not authorize adding it to the repository.

## Testing Guidelines

Tests use Rust's built-in `#[test]` harness. Follow existing names such as `test_parse_prefixes_*`, preferably adding descriptive case names. Add focused parser assertions and fixture-based regression cases for changed behavior. No numeric coverage threshold is configured. Preserve printer-capture regressions, and identify the model, DPI, firmware, and rendering profile associated with evidence; a capture does not prove compatibility with other devices or profiles.

`test-data/generate.sh` sends fixtures to Labelary and writes PDFs under `test-data/_gen/`; treat it as an explicit external-service check.

For renderer work, also follow these evidence rules:

- Use `render::profiles::SPECIFICATION` explicitly in specification tests and `ZD621_203_DPI` in printer-capture tests. `Options::default()` intentionally selects the ZD621 profile. Put measured firmware deviations behind independently selectable compatibility options, leaving the specification profile strict.
- Run `cargo test --locked -p zpl --test printer_accuracy --test conformance_preview` for accuracy changes, plus affected feature tests. Preserve exact underpaint/overpaint counts, dimensions, hashes, and error diagnostics. Inspect each changed baseline; never loosen it just to pass. Compare native canvases at their original origin without alignment, padding, cropping, or rescaling. See [accuracy contracts](docs/printer-accuracy.md).
- Distinguish full-image parity, symbol geometry, and decoded payload correctness. Non-text regions should be pixel-exact; text uses foreground IoU, not white-background agreement. IoU is better when higher; if error is defined as `1 - IoU`, 20% error means 80% IoU. Preserve stronger existing baselines rather than reducing them to a broad target.
- Capture with known printer state, native width, repeated controls, and provenance. Blank or nondeterministic previews are diagnostic observations, not positive correctness evidence. Validate font changes across sizes, rotations, and independent holdouts; do not replace captured strikes with a fitted or recovered outline font until calibration and measured accuracy justify it. See [font refinement](docs/font-refinement-results.md) and [Font 0 comparison](docs/font0-ttf.md).

For dependency changes, finish resolution and compatibility edits before starting expensive validation. Serialize Cargo commands sharing a target directory. If editor builds contend for its lock, use an isolated `CARGO_TARGET_DIR` rather than repeatedly stopping or pausing the user's background processes. If the `mbx` wrapper is implicated, compare with the underlying Cargo executable before starting another full rebuild.

Distinguish compilation from test execution. A successful `--no-run` build, interrupted suite, or pre-edit check is not a passing final test run. Diagnose prolonged silence using logs/process state; a macOS loader stall before the harness starts is not a test failure. Bound retries, stop only task-owned processes, and report precisely which checks passed, failed, or remained blocked. Nix evaluation (`nix flake check --no-build`) is not a package build or Linux VM test.

## Proxy, Browser & Printer Boundaries

- Preserve the proxy's separate positive rendering allowlist and parameter validation before cache lookup, persistence, or printer access, including refresh and historical cache hits. Parser or local-renderer support must not automatically authorize commands. Reject printer queries, stored-content access, configuration, downloads, and syntax/binary bypasses. See [admission policy](docs/proxy-validation.md).
- Persist accepted submissions and every outcome, including cache hits; keep errors retryable and invalidate mappings on failed refresh. Keep blocking SQLite work off the async executor and serialize printer preview operations. Use one proxy per printer because its preview object is shared. Preserve printer/configuration-scoped cache keys and namespace invalidation. See [cache contract](docs/proxy-cache.md).
- Use native fastrace with `log`/Logforth and propagate context across async/blocking tasks. Keep trace IDs in telemetry, not SQLite or response headers; do not reintroduce client-IP persistence. Exclude raw ZPL, image bytes, headers, credentials, and other request contents from telemetry. See [telemetry](docs/telemetry.md).
- Preserve TCP/Unix socket activation and minimal filesystem visibility in the NixOS module, including migration/runtime requirements. Read-only mounts are not read isolation. Validate confinement in the Linux VM test; proposals for outbound filtering are not evidence of implemented enforcement. See [NixOS hosting](docs/nixos.md).
- The Pages preview renders locally in a bounded, cancellable worker and uploads no ZPL. Keep errors visible, downloads synchronized with the current input, and project-relative asset URLs. Use the Wasm and browser checks in [web preview](docs/web-preview.md); verify the deployed page before claiming it works. Do not change repository visibility to resolve a Pages problem without authorization.
- Keep preview/testing separate from physical printing, stored-object changes, firmware uploads, and power cycling; confirm the current task authorizes any device-changing operation. Preserve firmware preview-by-default, identity/version checks, and no automatic upload retry. Use loopback tests for ordinary development; see [firmware updates](docs/firmware-updates.md).

## Commit & Pull Request Guidelines

Every new commit must use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) so [release-plz can classify changes](https://release-plz.dev/docs/changelog/format):

```text
<type>(<optional-scope>)!: <short imperative description>

Explain the previous behavior, the problem, and why this change fixes it.
Record relevant validation and any limitations.

BREAKING CHANGE: describe the incompatible behavior and migration, when applicable
```

The scope, `!`, and breaking-change footer are optional; use `!` or a `BREAKING CHANGE:` footer for an actual breaking change. Choose the type by the change's effect:

- `fix(render): preserve native tab stops` for a bug fix.
- `feat(zebra-firmware): add guarded network updates` for a new capability.
- `chore(deps): update compatible workspace dependencies` for routine upgrades; use `fix(deps): ...` when the upgrade addresses a concrete runtime bug or vulnerability.
- `ci: update pinned GitHub Actions`, `docs: clarify release preparation`, `test(code49): verify numeric decoder round trips`, or `refactor(render): simplify span handling` for the corresponding maintenance work.

Use a component as the scope, not the type: write `fix(render): ...`, not `render: ...`. Do not use an untyped subject such as `Update dependencies`. Release-plz uses `fix`, `feat`, and breaking-change markers to infer release intent; do not assume maintenance types prevent releases, and consult `release-plz.toml` for versioning policy. The current workflow prepares release PRs only; it does not publish crates. See `docs/releases.md`.

Before committing, inspect the staged diff and validate the subject format. Preserve unrelated dirty/untracked work, using partial staging when a manifest contains another task's edits. Keep commits focused and explain their reason in the body. Use the same format for squash-merge PR titles. Do not rewrite already-pushed history merely to normalize old subjects. When a push is requested, verify the remote branch points to the intended commit.

PRs should explain behavior changes, list validation performed, link relevant issues, and include screenshots for asset/UI changes. Keep transient build outputs, local databases, and credentials out of commits; retain intentionally versioned regression fixtures and generated reference assets.
