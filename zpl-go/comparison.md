# Go binding prototype comparison

Historical snapshot: the current package now defaults to wasm2go and ships
generated code/data separately via `-embed`. The measurements below describe the
original prototypes; see [the current package](README.md) for build instructions.

Measured October 9, 2026 on Linux amd64, AMD Ryzen 3 PRO 5350GE,
Go 1.27.2, Rust 1.98.1, purego 0.11.1, wazero 1.12.0, and wasm2go 0.4.16.
All four prototypes passed the same 159 native-oracle comparisons. At measurement time no default
backend was selected. The experiment makes **wasm2go a serious candidate** for a
self-contained Go package, with an explicit tradeoff in build time and size.

[Reproduction instructions](README.md), [runner](run.py), and
[retained samples, oracle hashes, and build results](results/linux-amd64.json).
The source base revision is recorded in the results; the prototype itself is an
uncommitted addition to that revision at measurement time.

## Performance

Warm timings are milliseconds per complete `Render` call, including input and
output copies, parsing, rendering, encoding, allocation, and wrapper decoding.
File I/O, output hashing, PNG validation, and oracle comparison are outside the
warm timer. These are medians of **15 batch averages**, each batch containing 20
renders, across three fresh processes per backend. They are not per-request
latency percentiles. Calls are sequential on one engine; `GOMAXPROCS=2`.
Backend order rotates between processes. All task compilation/tests finished
before the final samples; this was not a dedicated or CPU-pinned benchmark host.

| Workload | cgo | purego | wazero | wasm2go |
| --- | ---: | ---: | ---: | ---: |
| Boxes → PNG | 0.070 | 0.070 | 0.333 | 0.080 |
| Short text → PNG | 0.133 | 0.107 | 1.091 | 0.227 |
| 48 text fields → PNG | 1.267 | 1.241 | 10.872 | 2.533 |
| 48 text fields → SVG | 4.917 | 4.972 | 59.019 | 16.168 |
| Code 128 + QR → PNG | 0.087 | 0.086 | 0.931 | 0.184 |
| Dense graphic → PNG | 11.066 | 10.940 | 49.574 | 11.039 |
| Two labels → PDF | 0.120 | 0.123 | 1.506 | 0.414 |

These cases use the specification profile, initially 512×512 dots. Workload
names and exact inputs are defined in `run.py`; input and oracle hashes are
retained in the JSON snapshot. Default production PNG encoding is used equally
by every backend. No output compression or accuracy settings were changed to
favor a runtime.

cgo and purego are in the same performance class for this coarse interface.
The short-text difference is too small and the sample set too narrow to conclude
that one FFI mechanism is intrinsically faster. wasm2go ranges from approximately
native speed on graphics to 3.5× cgo on the small PDF case. Wazero ranges from
about 4.5× to 12.5× cgo. Both execute code derived from the **same Wasm file**;
wasm2go used its default optimization mode, without `-unsafe` or `-nanbox`.
Wazero used its amd64 compiler backend, not its interpreter.

## Startup, distribution size, and consumer builds

| Measurement | cgo | purego | wazero | wasm2go |
| --- | ---: | ---: | ---: | ---: |
| Fresh process: first result + validation + exit, median ms | 4.64 | 4.70 | 411.94 | 11.02 |
| Engine initialization within process, median ms | 0.002 | 0.392 | 398.655 | 2.437 |
| Stripped Go probe executable, MiB | 2.93 | 3.00 | 10.39 | 15.88 |
| Required renderer shared library, MiB | 5.59 | 5.59 | — | — |
| Executable + renderer library, MiB | 8.53 | 8.60 | 10.39 | 15.88 |
| Clean Go consumer build, seconds | 9.0 | 6.5 | 10.4 | 30.1 |
| Cached Go rebuild, seconds | 0.87 | 0.41 | 0.47 | 0.47 |

Fresh-process times are medians of five processes. OS file caches are warm;
there is **no persisted/shared wazero compilation cache**. This measures creating
an engine, rendering the boxes case, loading/comparing the oracle, decoding the
PNG, reporting, and exiting. The engine timer excludes executable loading and
native dependencies loaded before `main`, so it is not an equal measure of total
startup cost by itself. Wazero initialization includes compiling the guest.
Caching compiled code or sharing it across engines could reduce this cost; that
optimization was not measured.

Go builds used independent initially empty build caches, `-trimpath`, and
`-ldflags='-s -w'`. Downloads are excluded, standard-library compilation is
included. Consumer build times are single observations, not medians. The
executables include the same comparison/JSON/PNG-validation harness and are not
minimal application sizes. Native shared-library sizes are **unstripped**;
stripping a separate copy reduced it from 5.59 to 5.29 MiB. System libraries are
not included in the size totals.

The native binaries depend on this host's libc/linker setup. `ldd` also showed
libdl/libpthread/libc dependencies on the purego executable despite
`CGO_ENABLED=0`; its Rust shared library adds native dependencies. Wazero and
wasm2go executables were not dynamically linked. This is a material distinction
between avoiding a C build step and shipping an executable without native
shared-library dependencies.

The shared guest is 4,979,410 bytes (4.75 MiB). wasm2go produced 22,395,768 bytes
(21.36 MiB) of generated source. Additional producer-side steps, measured with a
fresh Cargo target directory, were:

- Native Rust shared library **and oracle executable**: 15.0 seconds.
- Rust Wasm module: 12.4 seconds.
- Wasm-to-Go translation: 7.4 seconds, in addition to the Wasm build.

A publisher can perform these steps once and distribute the resulting native
libraries, Wasm, or generated Go. Consumers need Rust only if building those
artifacts themselves. The prototype runner intentionally builds all artifacts
locally to make the comparison reproducible.

## Correctness and portability

All four backends passed **132 successful-output cases and 27 error cases**:
159 comparisons per backend, 636 total. Successful PNG, SVG, PDF and grayscale
bytes match native Rust exactly, as do dimensions, label counts and warnings.
Error statuses, diagnostics and byte offsets also match. Successful PNGs are
additionally decoded by Go's PNG decoder and checked against returned dimensions.
There are no tolerance adjustments, alignment changes, or updated printer
baselines.

The corpus covers geometry/compositing, curves, text/rotations, barcodes, raw
binary graphics including NUL and command-prefix bytes, dense graphics, multiple
labels, invalid graphic checksums, empty input and invalid options. It runs under
SPECIFICATION, ZD621, and ZQ610 Plus profiles. Existing capture inputs are reused
with their provenance retained in the source repository. This is runtime parity,
not an independent demonstration of printer fidelity or barcode decoding.

Each backend passed Go tests for oracle parity, result ownership across calls
and engine closure, concurrent callers, closed-engine handling, malformed response
packets and pre-canceled contexts. A separate wazero test passed interruption of
an in-progress graphic render and rejection of subsequent use of that closed
instance. Three Rust tests passed for native-output parity, binary input, FFI
ownership and invalid pointers/length rejection. Focused Clippy with warnings
denied and workspace rustfmt checks passed. The entire Rust workspace suite was
not run: core renderer behavior was not changed.

| Prototype | Linux amd64 runtime/tests | Linux arm64 build | macOS arm64 build | Windows amd64 build |
| --- | --- | --- | --- | --- |
| cgo | Passed | Not attempted | Not attempted | Not attempted |
| purego | Passed | Passed | Passed | Loader not implemented |
| wazero | Passed | Passed | Passed | Passed |
| wasm2go | Passed | Passed | Passed | Passed |

All cross builds used `CGO_ENABLED=0`. They were not executed. Purego's cross
builds validate only the Go side: matching native libraries were not cross-built.
The command pattern, from `zpl-go`, was:

```sh
CGO_ENABLED=0 GOOS=darwin GOARCH=arm64 go build -trimpath \
  -ldflags='-s -w' -tags backend_wasm2go -o artifacts/cross-darwin ./cmd/probe
```

## What this supports choosing

- **cgo:** the conventional native integration path, with native speed and C
  toolchain/linker requirements. Static linking was not prototyped.
- **purego:** preserves native speed and removes the consumer C compiler on tested
  platforms. It still needs native library packaging, loader handling and an
  OS/architecture/libc release matrix. It is attractive when a shared library is
  acceptable, rather than as a solution for a fully self-contained executable.
- **wazero:** the smallest self-contained executable of the two Wasm choices,
  with tested in-flight interruption and explicit guest-memory limits. This
  prototype has substantial initialization and throughput costs. Evaluate
  compilation caching and the application's actual latency budget before making
  it the default.
- **wasm2go:** the strongest measured candidate when consumers want ordinary Go
  builds, no shared library, and low startup cost. It is substantially faster
  than wazero here, at the cost of a larger executable/source distribution and
  slower first compilation. Native-speed graphics do not generalize to text/SVG.
  Translation maturity and wider correctness coverage remain adoption risks.

Before selecting a production default, extend the intended public API (especially
fonts/callbacks and retained scenes), execute tests on the other target platforms,
and validate a wider corpus including custom TrueType fonts, resource exhaustion,
and application-specific label sizes. Measure concurrency, memory high-water
marks and cached Wasm initialization if those affect the deployment. The current
experiment does not establish those properties. Nothing in this result requires
maintaining all four backends in a production package.
