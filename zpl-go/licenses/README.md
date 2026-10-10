# Notices for generated renderer artifacts

The Wasm module and translated Go package include compiled Rust code. The root
package LICENSE covers this repository's OSL-3.0 code. This directory preserves
upstream notices for the pinned third-party Rust dependencies in the Wasm build,
the Rust 1.98.1 standard library, and wasm2go 0.4.16's generated helper code.
Some listed code may be removed by optimization. Go runtime dependencies retain
the notices in their own modules.

Dependency list: `cargo tree --locked -p zpl-go-prototype --target
wasm32-unknown-unknown --edges normal`. Notices are copied unchanged from the
registry packages, Rust toolchain distribution, and wasm2go module. Review and
refresh them when updating the toolchain or dependencies used for generation.
