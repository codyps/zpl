# Rendering performance

[Dashboard](https://codyps.github.io/zpl/perf/) · [Label preview](https://codyps.github.io/zpl/)

This follows the [codyps/coarsetime measurement pattern](https://github.com/codyps/coarsetime/tree/main/benchmarks):
identical base/head harnesses, alternating rounds on the same worker, raw evidence,
paired statistical reporting, and append-only main history. The ZPL suite runs only on GitHub-hosted
Ubuntu 24.04 Linux runners with stable Rust; publication uses the same hosted
runner label. It measures the local renderer, without a printer or external
rendering service.

## Workloads and interpretation

Nine cases cover text, Code 128/QR barcodes, and composited shapes. Each workload
uses an explicit `ZD621_203_DPI` profile on an 812 × 600 canvas:

- `scene`: parsing and scene construction, including barcode encoding.
- `raster`: rasterization of an already prepared scene into a newly allocated buffer.
- `total`: scene construction and rasterization together.

These are fixed synthetic workloads, not printer accuracy evidence or a complete
model of real labels. PNG encoding, filesystem access, fixture decoding, network,
and comparison/hashing are excluded. Timings include allocation and destruction.
Each binary validates one nonblank label at the expected dimensions before timing.
Existing renderer correctness and printer-capture tests remain separate CI checks.

The runner builds a standalone harness against each revision's `zpl` path using
that revision's locked dependency versions. It rejects unexpected dependency
resolution changes. Both binaries use the same harness and compiler and separate
build directories. PR revisions are the event's exact base and head, not the
synthetic merge commit. There are ten rounds of at least 200 ms per case/revision,
with both case and revision order reversed every round. Every process warms its
renderer while validating before timing; results describe warm operations.

PR job summaries and `summary.md` show paired head/base percentage changes.
Informational flags require at least 10% and 1 µs median paired change, with a
99% percentile-bootstrap interval excluding zero (5,000 deterministic resamples).
Both improvements and regressions are flagged; positive means slower. These are
not merge gates or guarantees, and there is no multiple-comparison correction.
Same-worker pairing reduces machine drift; thermal and scheduling noise remain.
Thresholds live in `config.json`. PR summaries use the PR's tooling and are advisory,
not a trusted authorization check. The comment publisher uses main's thresholds.

## PR comments

`Comment on rendering benchmarks` runs trusted default-branch scripts after a
successful PR measurement workflow, using its existing artifacts. A PR receives
one sticky `github-actions[bot]` comment when a stage crosses the thresholds.
Both improvements and regressions are reported. Later runs update that comment,
including clearing old alerts when no stage qualifies; an initial quiet run does
not post. The comment links the raw evidence and historical dashboard.

The publisher validates the run/attempt, both revisions and their samples, PR
association when available, current head/base SHAs, and fork/branch identities.
Closed PRs, stale heads/bases and results older than the existing comment are
skipped. Rebase or trigger a new PR event if the base advances during measurement.
Reporting thresholds come from `main`; a PR cannot lower its own alert threshold.
Changes to the round count or measurement duration require the trusted protocol
to be updated too. Measurement workers explicitly select stable Rust for both
checkouts and retain read-only repository permissions.

The comment workflow has only actions/contents read and pull-requests write
permissions. Artifacts are parsed as data, never executed, and PR scripts are
never checked out by the publisher. It is separate from history publication, so
comments do not trigger the Pages refresh that follows a successful history run.
The new workflow must exist on `main` before GitHub can invoke it; the PR adding
it will have benchmark artifacts and job summaries, but no automatic comment.

## History and Pages

`Rendering benchmarks` runs on PRs, main pushes, Mondays, and manual dispatch.
A separate GitHub-hosted Linux job tests the dashboard under the `/zpl/perf/`
project path and saves a browser screenshot; it does not run on the timing worker.
Workers have read-only permissions and retain JSON, raw iteration/nanosecond logs,
and summaries as artifacts for 30 days, including partial logs on failure.
The trusted `Publish rendering benchmarks` workflow runs main-branch code only
and validates successful main artifacts, exact run/attempt/commit identities,
finite samples, and the expected Linux runner. PR results never enter history.

Main records and raw logs accumulate on the `benchmarks` branch, created on first
publication. Run/attempt/runner filenames preserve repeated runs. Publication
retries normal fast-forward pushes at most three times and never force-pushes.
`data/index.json` is derived from the archived records. There is no history limit;
the dashboard currently loads the whole index. Raw evidence stays downloadable.

The existing **Preview website** workflow owns the only Pages deployment. It
combines the preview at `/zpl/` with the dashboard at `/zpl/perf/`, and fetches the
latest history during assembly. Main pushes, manual runs, and successful history
publication rebuild that combined artifact. If history does not exist yet, the
page explicitly says no measurements have been published. Fetch failures fail the
build rather than publishing empty history. The history branch contains only data;
it cannot replace or execute the website's build scripts.

The chart separates exact Rust versions, harness/protocol hashes, OS/CPU/image,
and build flags. Platform and stage selectors never average unlike environments.
Historical runs are observational, not paired comparisons between commits. A
protocol change starts a new series. No sampled result is seeded from a developer
machine into the production history.

Pages already uses **GitHub Actions** as its source. No visibility change, service
account, or new secret is needed. The first publisher must be merged to main before
GitHub can invoke it. After merging, the main benchmark run should create history
and trigger the combined deployment. PRs test the page and measurement tooling.

## Local checks

Python 3.11+, Rust/Cargo, and Node.js are required:

```sh
python3 -m unittest discover -s benchmarks -p 'test_*.py' -v
node --test benchmarks/dashboard.test.mjs
rustfmt --edition 2021 --check benchmarks/harness.rs
python3 benchmarks/run.py --base /path/to/base --head /path/to/head --output /tmp/zpl-perf-new
```

The output directory must not already exist. Use the same checkout for both paths
to run an A/A noise check. Local measurements are not automatically published.
See [web preview checks](../docs/web-preview.md) for combined site validation.

## Remaining differences from coarsetime

The existing collection already covers alternating same-worker measurements,
paired statistics, raw evidence, exact environment metadata, job summaries,
scheduled/manual runs, durable history and an interactive dashboard. PR comments
add the remaining reporting step, including clearing alerts and stale-run checks.

Useful measurement extensions are PNG encoding (currently excluded, despite
being part of rendering to PNG), allocation counts for text-heavy labels, and a
Wasm benchmark for the browser preview. Stage timings do not provide an independent
machine-noise control such as coarsetime's stdlib workloads. These require new
measurements; native timing data cannot substantiate Wasm or allocation claims.
Linux-only GitHub-hosted workers are intentional. There is no multi-compiler
matrix; exact stable Rust versions are recorded and kept separate in the dashboard.

References: [Rust black_box](https://doc.rust-lang.org/std/hint/fn.black_box.html),
[Cargo lockfiles](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html),
[workflow_run trust boundaries](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run),
[custom Pages workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
