# zpl

Parse Zebra Programming Language (ZPL) and render labels locally to PNG or SVG.
No printer or external rendering service is needed.

## Quick start

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
zpl = "0.1"
```

Render a 400 × 180-dot label with text and a Code 128 barcode:

```rust
use zpl::{
    output::{Adapter, Png, Svg},
    render, Options,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = render(
        b"^XA^PW400^LL180^FO20,20^A0N,32,0^FDHello, ZPL!^FS\
          ^FO20,70^BY2^BCN,60,Y,N,N^FD123456^FS^XZ",
        Options::default(),
    )?;

    for warning in &document.warnings {
        eprintln!("warning: {warning}");
    }

    let label = &document.labels[0];
    let png = Png.encode(label)?;
    let svg = Svg.encode(label)?;
    // Save the bytes with std::fs::write("label.png", &png)?;
    // Or return them from your application as image/png or image/svg+xml.
#   assert_eq!(document.labels.len(), 1);
#   assert_eq!((label.width, label.height), (400, 180));
#   assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
#   assert!(std::str::from_utf8(&svg)?.starts_with("<svg "));
    Ok(())
}
```

[![Rendered quick-start label: Hello, ZPL! above a Code 128 barcode](https://raw.githubusercontent.com/codyps/zpl/main/docs/examples/quick-start.png)](https://raw.githubusercontent.com/codyps/zpl/main/docs/examples/quick-start.png)

## Working with labels

`render` accepts ZPL bytes and returns a document containing one scene per label.
For input with multiple `^XA` / `^XZ` formats, iterate over `document.labels` and
encode each scene. Import the `Adapter` trait to use `Png.encode` or `Svg.encode`;
both return a `Vec<u8>`.

Coordinates and dimensions are in printer dots. `^PW` and `^LL` set the label
width and height; `Options` supplies defaults for dimensions and DPI.
`Options::default()` selects the ZD621 203-DPI compatibility profile. Other
profiles, including the strict `SPECIFICATION` profile, are available in
`zpl::render::profiles`.

The renderer supports a subset of ZPL. Unsupported rendering semantics return
errors; font approximations and other caveats appear in `document.warnings`.
Check warnings when printer fidelity matters. The separate `zpl::parse` module
preserves command bytes, including unknown commands and binary payloads; parsing
does not imply that a command can be rendered.

- [API documentation](https://docs.rs/zpl)
- [Renderer coverage and options](https://github.com/codyps/zpl/blob/main/docs/local-renderer.md)
- [Barcode support](https://github.com/codyps/zpl/blob/main/docs/barcodes.md)
- [Try the browser preview](https://codyps.github.io/zpl/)

## License

Licensed under the [Open Software License 3.0](https://github.com/codyps/zpl/blob/main/LICENSE).
