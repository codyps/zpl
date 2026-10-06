# Rendering performance

[Dashboard](https://codyps.github.io/zpl/perf/) · [Label preview](https://codyps.github.io/zpl/)

This follows the [codyps/coarsetime measurement pattern](https://github.com/codyps/coarsetime/tree/main/benchmarks):
identical base/head harnesses, alternating rounds on the same worker, raw evidence,
paired statistical reporting, and append-only main history. The ZPL suite runs only on GitHub-hosted
Ubuntu 24.04 Linux runners with stable Rust; publication uses the same hosted
runner label. It measures the local renderer, without a printer or external
rendering service.

## Workloads and interpretation

Twelve case/stage measurements cover scalable text, native bitmap text,
Code 128/QR barcodes, and composited shapes. Each workload
uses an explicit `ZD621_203_DPI` profile on an 812 × 600 canvas:

- `scene`: parsing and scene construction, including barcode encoding.
- `raster`: rasterization of an already prepared scene into a newly allocated buffer.
- `total`: scene construction and rasterization together.

`text` uses retained scalable font-0 strikes. `bitmap-text` uses all native
bitmap faces A–H (including the C/D alias) and GS at native and 2x dimensions,
with letters, digits, punctuation and spaces. This exercises source lookup,
packed bit scans, joined text paths, scaling, and rasterization through the
renderer. Preflight checks require ink in each of the 18 face/size regions,
outside the timed loop. Both revisions run the identical workload through public
renderer APIs, allowing comparisons between old assets and the shared font crate.
These are warm rendering measurements, not extraction or cold font-loading timings.

These are fixed synthetic workloads, not printer accuracy evidence or a complete
model of real labels. PNG encoding, filesystem access, fixture decoding, network,
and comparison/hashing are excluded. Timings include allocation and destruction.
Each binary validates one nonblank label at the expected dimensions before timing.
Existing renderer correctness and printer-capture tests remain separate CI checks.

The runner builds a standalone harness against each revision's `zpl` path using
that revision's locked dependency versions. It rejects unexpected dependency
resolution changes. Both binaries use the same harness and compiler. Each revision
is exported from Git into the **same physical source path**, with the harness and
target directory also recreated at the same paths. A clean build between revisions
prevents stale outputs from hiding changes. Separate checkout paths used to change
Cargo's crate metadata and binary layout even for identical sources, producing
false performance alerts; see [Cargo issue 7645](https://github.com/rust-lang/cargo/issues/7645).
PR revisions are the current target commit and GitHub's test merge of the PR into
that target. A preflight job reads the live PR, requires confirmed mergeability,
and checks that the test merge's parents are exactly that target and the event's
PR head. It pins both checkouts by SHA, so an advanced target is included even
when rerunning an older event. The runner verifies the checked-out merge parents
before building. Conflicting, closed, superseded, or retargeted PRs skip the timing
job; unknown mergeability or a stale test merge is polled at most six times, two
seconds apart, then skipped. A new PR synchronization event after resolving
conflicts can run benchmarks again. GitHub itself does not start `pull_request`
workflows for conflicting PRs; see [pull request events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request).
There are ten rounds of at least 200 ms per case/revision,
with both case and revision order reversed every round. Every process warms its
renderer while validating before timing; results describe warm operations.

Each build saves its executable, SHA-256 hashes, resolved lockfile, and input
inventory. The inventory covers the resolved dependency graph and features, every
file in local dependency packages (including fonts and manifests), ancestor workspace
manifests, and the standalone harness/lockfile. It excludes unrelated workspace
members such as `zpl-cmd`; it does **not** strip version numbers from benchmarked
packages. These can affect generated code through `env!("CARGO_PKG_VERSION")`.

When both inputs and independently built executables match, the report says
**No relevant input changes**. Matching executables with different inputs are also
reported as an A/A noise control. All ten paired rounds and observed timing changes
are retained, but cannot produce source regression or improvement flags. Matching
inventoried inputs with different executables produce an explicit **inconclusive**
result: investigate build nondeterminism or inputs outside the inventory. Ordinary
comparisons require changed inputs and changed executables. This check does not
prove reproducibility for every possible build script or external input.

PR job summaries and `summary.md` show paired merge/target percentage changes.
Artifacts retain `base`/`head` sample keys for the target/merge and record the
original PR head separately as `pr_head_sha`.
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
An A/A or inconclusive result also clears an existing alert. Legacy artifacts
without build provenance remain readable for history, but the updated publisher
does not turn their path-biased timings into PR alerts. The new comment behavior
starts after the publisher changes reach `main`; the PR's job summary uses the
updated runner immediately. The additive artifact fields retain schema 1 compatibility.

The publisher validates the run/attempt, both revisions and their samples, PR
association when available, current head/target/merge SHAs, mergeability, and
fork/branch identities. Closed or conflicting PRs, stale heads/targets/merges and
results older than the existing comment are skipped. Rerun or trigger a new PR
event if the target advances during measurement. Skipped timing jobs do not
download artifacts or update comments. Old PR artifacts that measured an unmerged
head are rejected; main history remains compatible.
Reporting thresholds come from `main`; a PR cannot lower its own alert threshold.
Adding `bitmap-text` changes the harness/protocol hash and starts a new history
series. Older records remain visible without fabricated bitmap samples, and the
dashboard discovers workloads across the full history. The trusted comment
publisher on `main` must include the expanded workload list before it can accept
12-stage artifacts; until this change merges, use the PR measurement job summary
and raw artifacts rather than the older sticky comment.

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
summaries, input inventories, resolved locks, and both executables as artifacts
for 30 days, including partial evidence on failure. Publishers never execute the
saved binaries or consume inventories as scripts.
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

The output directory must not already exist and needs room for the exported source
tree and clean release build. Checkouts must have no tracked edits; commit local
changes before measuring, since snapshots come from Git. Use the same checkout for
both paths to run an A/A noise check. Local measurements are not automatically published.
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
