# Go package

`github.com/codyps/zpl/zpl-go` provides one Go API for the Rust ZPL renderer.
**wasm2go is the default backend**. Generated Go, its embedded data, and the Wasm
module are included, so ordinary Go builds need neither Rust nor code generation.
The API is experimental. Releases follow the renderer version and publish the
Go module with a `zpl-go/v<version>` repository tag, then register it with the
public Go proxy. After publication, install with
`go get github.com/codyps/zpl/zpl-go@v<version>`; see the
[release procedure](../docs/releases.md#go-module-releases).

## Build-time backends

| Build tag | Implementation | Consumer requirements |
| --- | --- | --- |
| None, or `backend_wasm2go` | Translated Go code | Go only |
| `backend_wazero` | Embedded Wasm executed by wazero | Go only |
| `backend_purego` | Dynamically loaded native renderer | Matching shared library; Linux/macOS loader |
| `backend_cgo` | cgo calls the native renderer | C toolchain and matching shared library |

From this directory, these commands build the same application:

```sh
go build -o artifacts/probe ./cmd/probe
CGO_ENABLED=0 go build -tags backend_wasm2go -o artifacts/probe ./cmd/probe
CGO_ENABLED=0 go build -tags backend_wazero -o artifacts/probe ./cmd/probe
CGO_ENABLED=0 go build -tags backend_purego -o artifacts/probe ./cmd/probe
CGO_ENABLED=1 go build -tags backend_cgo -o artifacts/probe ./cmd/probe
```

The last command also needs native linker configuration below. Replace
`./cmd/probe` with your own application package when consuming this module.
Tags select private implementations; your imports, `New`, `Render`, `Close`,
`Request`, `Response`, and `RenderError` types stay the same. Conflicting explicit
tags fail compilation. Selecting cgo with `CGO_ENABLED=0`, or purego on a platform
without a loader, also fails with a specific diagnostic. No runtime fallback
occurs. The default works with either `CGO_ENABLED=0` or `CGO_ENABLED=1`.

## API

```go
import zpl "github.com/codyps/zpl/zpl-go"

// Inside a function with a context.Context named ctx:
engine, err := zpl.New(ctx)
if err != nil {
    return err
}
defer engine.Close(context.Background())
result, err := engine.Render(ctx, zpl.Request{
    Input: []byte("^XA^FO10,10^GB100,80,2^FS^XZ"),
    Format: zpl.PNG, Profile: zpl.Specification,
    Width: 512, Height: 512,
})
```

The [executable example](example_test.go) is tested under every backend.
PNG, SVG, multipage PDF, grayscale pixels, all three native profiles, dimensions,
label counts, warnings, and error byte offsets are supported. PNG/SVG/grayscale
return the first label; PDF returns all labels. Input is binary-safe and capped
at 1 MiB. Dimensions must be 1..4096. DPI comes from the profile. Native render
and output work budgets remain enabled.

Results belong to Go and survive subsequent calls and engine closure. One engine
serializes calls; use separate engines for parallel rendering. `Close` is
explicit and idempotent. Native allocations are freed after each call; Rust
retains no Go pointers. Errors contain the same statuses and diagnostics across
backends for the tested corpus.

All backends check cancellation before rendering. Only wazero interrupts an
in-progress render; cancellation closes its instance, so discard/close that
engine. Other backends finish an in-progress call. Wasm and translated Go have a
256 MiB linear-memory limit. Native execution retains renderer work budgets but
has no equivalent process-memory ceiling. Translated Wasm traps retire the
instance. Native unwinding panics become errors; native aborts/OOM are not caught.

This adapter does not yet expose the entire `zpl-c` API: standalone parsing,
retained scenes, individual compatibility flags, font registration/callbacks,
caller-owned raster targets and per-request budget overrides are outside its
current scope. Core renderer behavior and dependencies are unchanged.

## Native library setup

The common native adapter is `zpl-go/bridge` (`zpl-go-prototype` Cargo package).
Build it from the full repository, or supply an equivalent prebuilt library for
the target platform. The Go module alone does not contain its sibling Rust crates.
On Linux, from the repository root:

```sh
cargo build --locked --release -p zpl-go-prototype
export ZPL_GO_LIBRARY="$PWD/target/release/libzpl_go_prototype.so"
export CGO_LDFLAGS="-L$PWD/target/release -Wl,-rpath,$PWD/target/release"
cd zpl-go
CGO_ENABLED=0 go test -tags backend_purego ./...
CGO_ENABLED=1 go test -tags backend_cgo ./...
```

Adjust paths if using `CARGO_TARGET_DIR`. Purego uses `ZPL_GO_LIBRARY` at runtime;
cgo uses the link/runtime paths. macOS uses `libzpl_go_prototype.dylib`. The current
purego loader supports Linux/macOS, not Windows. Native artifacts are tied to
their OS, architecture and libc/toolchain. No native binary release matrix or
static cgo linking is provided by this change.

## Tests and artifact maintenance

Default and wazero tests require only Go and the included files:

```sh
cd zpl-go
CGO_ENABLED=0 go test -count=1 ./...
CGO_ENABLED=0 go test -count=1 -tags backend_wazero ./...
```

The fixed corpus in `testdata` contains input bytes and 159 native-oracle hashes.
Tests compare complete encoded output and metadata, including error diagnostics,
under all three profiles; no local oracle generation is needed. Tests also cover
ownership, concurrent callers, closure, invalid responses and cancellation.
Capture input provenance remains in
`../zpl/tests/fixtures/zq610-plus-v1/`. These are runtime-equivalence tests, not new
printer-accuracy or independent barcode-decoding claims.

From the repository root:

```sh
python3 zpl-go/check_selection.py
python3 zpl-go/check_package.py
```

The first command checks all 16 backend-tag combinations, disabled cgo, an
unsupported purego platform, and the default with cgo enabled. The second tests a
copy containing only distributable files, without ignored artifacts or a C
compiler. CI runs these checks and all four explicit backends.

Maintainers regenerate artifacts from the full Rust workspace with Rust 1.98.1,
the `wasm32-unknown-unknown` target, Go 1.27.2 and Python 3:

```sh
python3 zpl-go/generate.py
python3 zpl-go/generate.py --check
# Equivalent regeneration from zpl-go: go generate
```

The generator installs pinned wasm2go v0.4.16 into a temporary directory and
translates the same `guest.wasm` used by wazero. It splits generated code/data with
`-embed`; build constraints exclude both when another backend is selected.
Source paths are normalized, and `generated.json` records hashes and versions.
`--check` compares rebuilt artifacts without modifying shipped files. Review
changes to `guest.wasm`, `internal/translated/module.go`, `module.dat`, and
`generated.json` together. Update [upstream notices](licenses/README.md) when
changing compiler/dependency versions. Tests do not rewrite golden hashes.

`LICENSE` is a regular file because Go module archives omit symlinks. The package
includes generated renderer files intentionally; caches, local native libraries,
oracle output images and executables remain ignored.

## Benchmarks

The [comparison](comparison.md) and [sample data](results/linux-amd64.json) are
historical measurements of the initial prototypes, before the default was
selected and the generated data moved to a separate embedded file. They have not
been remeasured for this packaging change. From the repository root,
`python3 zpl-go/run.py` rebuilds the native oracle/backends and repeats the
comparison. It regenerates shipped artifacts; review the resulting diff. See
`run.py --help` for measuring previously validated binaries. A full run needs
Rust, the Wasm target, Go, Python and a C toolchain. It uses an isolated Cargo
target by default and fresh Go consumer caches; it does not retry failing tests.

Upstream contracts: [cgo pointers](https://pkg.go.dev/cmd/cgo#hdr-Passing_pointers),
[purego lifetimes](https://pkg.go.dev/github.com/ebitengine/purego#RegisterFunc),
[wazero configuration](https://pkg.go.dev/github.com/tetratelabs/wazero#RuntimeConfig),
[wasm2go](https://github.com/ncruces/wasm2go), and
[Rust Wasm](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html).
