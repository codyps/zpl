# Repository Guidelines

## Project Structure & Module Organization

This Rust 2021 Cargo workspace contains three crates:

- `zpl/`: ZPL parsing and command/format types. Parser unit tests live in `src/parse/test.rs`; integration tests live in `tests/`.
- `zebra-http-api/`: Zebra printer HTTP rendering client, with a `zebra-render` example.
- `zpl-proxy-api/`: Axum proxy, SQLite cache, Diesel models and migrations, and browser assets in `assets/`.

Shared ZPL fixtures and optional TOML metadata live in `test-data/`. The `zpl-to-svg` example is unfinished.

## Build, Test, and Development Commands

Run workspace commands from the repository root:

- `direnv exec .`: enter the development shell with Rust, Clippy, rustfmt, Diesel CLI, and SQLite; sets `ROOT_PATH` and `DATABASE_URL`. Uses the nix flake, but caches it.
- `cargo build --workspace`: build all crates.
- `cargo test --workspace`: run workspace tests.
- `cargo test -p zpl --lib`: run parser unit tests.
- `cargo fmt --all -- --check`: check Rust formatting.
- `cargo clippy --workspace --all-targets`: inspect lint diagnostics.
- `nix fmt`: format the Nix configuration.

To run the proxy, enter `direnv exec .`, create the database directory with `mkdir -p _db`, then:

```sh
cd zpl-proxy-api
diesel migration run
cargo run -- --zd621-url http://printer.local/ --bind-addr 127.0.0.1:3000
```

Replace the printer URL with your device address. Run from this crate directory because static assets use a relative path.

## Coding Style & Naming Conventions

Use rustfmt defaults, including four-space Rust indentation. Use `snake_case` for functions/modules and `UpperCamelCase` for types. Keep parsing, printer transport, and proxy concerns in their respective crates. Add database changes as timestamped Diesel migrations with `up.sql` and `down.sql`; keep `src/schema.rs` synchronized.

## Testing Guidelines

Tests use Rust's built-in `#[test]` harness. Follow existing names such as `test_parse_prefixes_*`, preferably adding descriptive case names. Add focused parser assertions and fixture-based regression cases for changed behavior. No numeric coverage threshold is configured; the fixture harness remains minimal.

`test-data/generate.sh` sends fixtures to Labelary and writes PDFs under `test-data/_gen/`; treat it as an explicit external-service check.

## Commit & Pull Request Guidelines

History uses short, informal subjects, including `cargo update` and `wip`; no strict convention is established. Prefer concise, descriptive subjects identifying the affected component. PRs should explain behavior changes, list validation performed, link relevant issues, and include screenshots for asset/UI changes. Keep generated files, local databases, and credentials out of commits.
