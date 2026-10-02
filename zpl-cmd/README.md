# zpl-cmd

Render ZPL labels locally to PNG, SVG or PDF. No printer or external service is
required.

From the repository root:

```sh
cargo run --locked -p zpl-cmd -- render input.zpl output.pdf
cargo run --locked -p zpl-cmd -- render --help
```

Or install from the checkout:

```sh
cargo install --locked --path zpl-cmd
zpl-cmd render input.zpl output.png
```

The output extension selects the format. PNG and SVG require one label; PDF
includes one page per label, preserving physical size using the scene DPI.
`--profile` selects `zd621` (default), `zd621-preview`, `zq610-plus`, or
`specification`. `--explicit-qr-mask` honors QR mask operands instead of selecting
masks using printer compatibility behavior. Options can precede or follow the
file paths. Warnings and errors go to stderr; failures return a nonzero exit code.

See [local rendering](../docs/local-renderer.md) for supported ZPL, profiles,
resource limits, and output behavior. Licensed under [OSL-3.0](LICENSE).
