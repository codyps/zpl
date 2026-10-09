# @codyps/zpl

Render ZPL to PNG, SVG, or multipage PDF locally in Node.js using the Rust
`zpl` renderer compiled to WebAssembly. The packed package includes its Wasm
binary, has no runtime npm dependencies, and requires no Rust installation,
bundler, asynchronous initialization, printer, or network service.

## Build and install from this checkout

Requires Node.js 22+, npm, Rust, the `wasm32-unknown-unknown` target, and
`wasm-bindgen-cli` 0.2.128 (aligned with `zpl-wasm/Cargo.toml`):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
cd zpl-node
npm run build
npm test
npm pack
# In your application, install the generated archive:
npm install /path/to/zpl-node/codyps-zpl-0.1.0.tgz
```

The repository development shell includes `lld` and the exact
`wasm-bindgen-cli` 0.2.128 package, including on Intel macOS. After updating the
flake, reload direnv (or enter `nix develop`) to pick up these tools. The Wasm
Rust target is still required.

The Cargo package is named `wasm-bindgen-cli`, but its executable is
`wasm-bindgen`. Check `wasm-bindgen --version`: it must be exactly `0.2.128`.
If a Nix/direnv shell provides an older version, put Cargo's installed binaries
first when invoking npm, for example:

```sh
PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH" npm run build
```

Use the same PATH prefix for `npm pack` or `npm publish` in that shell.

`npm pack` rebuilds the Wasm binary from the workspace's locked dependencies.
The archive is ready to install; this does not publish it to npm.

Automatic publishing follows stable Rust `zpl` releases and uses the same version
number. See [release setup](https://github.com/codyps/zpl/blob/main/docs/releases.md#npm-publishing-setup) for the
one-time npm account and trusted-publisher configuration.

## Usage

```js
import { render, libraryVersion } from '@codyps/zpl';
import { writeFileSync } from 'node:fs';

const result = render('^XA^FO20,20^A0N,30,30^FDHello^FS^XZ', {
  format: 'png',
  width: 812,
  height: 1218,
  dpi: 203,
  profile: 'specification',
});
writeFileSync('label.png', result.data);
console.log(libraryVersion(), result.warnings);
```

CommonJS: `const { render } = require('@codyps/zpl')`.
TypeScript declarations are included; Node TypeScript applications should have
`@types/node` installed.

`render(input, options?)` accepts a string (encoded as UTF-8), `Buffer`, or
`Uint8Array`. Use bytes for binary graphics or legacy character encodings;
select the matching ZPL `^CI` mode in the source. It returns:

- `data`: an owned `Buffer`; for SVG text use `data.toString('utf8')`.
- `format`: `png` (default), `svg`, or `pdf`.
- `width`, `height`: actual dimensions in dots of the selected label, or the
  first page for multipage PDF.
- `labels`: total number of labels in the input.
- `warnings`: renderer warning strings for the whole document.

Options `width`, `height`, and `dpi` override the selected profile's initial
settings. ZPL commands and profile behavior can change the resulting dimensions.
`profile` supports `zd621` (the Rust default, 832 × 1218 at 203 DPI),
`specification` (812 × 1218 at 203 DPI), and `zq610-plus` (384 × 2030 at 203 DPI).
Printer profiles reflect measured preview behavior, not universal device parity.

`label` is a zero-based index. PNG and SVG select label 0 by default. PDF includes
all labels unless an index is supplied:

```js
const { data } = render('^XA^FDOne^FS^XZ^XA^FDTwo^FS^XZ', { format: 'pdf' });
writeFileSync('labels.pdf', data);
```

## Parsing

`parse(input, options?)` frames a complete ZPL byte stream without rendering it:

```js
import { parse, ParseError } from '@codyps/zpl';

const { elements, syntax } = parse('^XA^FO20,20^FDHello^FS^XZ');
for (const { kind, offset, data } of elements) {
  console.log(kind, offset, data);
}
const next = parse('^XA^XZ', { syntax });
```

Input accepts the same string, `Buffer`, and `Uint8Array` types as rendering.
Each element has a `kind` (`before_first_command`, `format_command`,
`control_command`, or `control_character`), a byte `offset`, and an owned
`data` Buffer. Concatenating all element buffers reproduces the input exactly,
including whitespace, unknown commands, non-UTF-8 bytes, and binary payloads.
Offsets in string input count UTF-8 bytes, not JavaScript characters.

`options.syntax` accepts optional numeric bytes `formatPrefix`,
`controlPrefix`, and `delimiter` (0–255), defaulting to 94 (`^`), 126 (`~`),
and 44 (comma). The result includes the final syntax after in-stream changes.
You can carry it into the next complete stream; this API does not buffer partial
network chunks.

A framing failure throws `ParseError` with `offset` and `kind` properties;
no partial result is returned. Parsing does not validate operands, decompress
downloads, or authorize commands for a printer. It does not apply rendering
budgets such as the 1 MiB input limit. See
[parser coverage](https://github.com/codyps/zpl/blob/main/docs/parser-coverage.md)
for framing rules and limitations.

## External and inline fonts

Use `resolveFont` to supply quadratic TrueType bytes for virtual printer filenames
selected by `^A@` or assigned to an ID by `^CW`:

```js
import { readFileSync } from 'node:fs';
import { render, resolveRomFont } from '@codyps/zpl';

const brand = readFileSync('brand.ttf');
const result = render('^CWZ,R:BRAND.TTF^XA^FO20,20^AZN,32,24^FDHello^FS^XZ', {
  resolveFont(name) {
    if (name === 'R:BRAND.TTF') return brand;
    return resolveRomFont(name); // Explicitly retain bundled ROM lookup.
  },
});
```

The synchronous callback receives a validated uppercase `device:path`; omitted
devices become `R:`. It runs once per resolved name per render. Return a `Buffer`
or `Uint8Array`; the renderer copies and validates the bytes. Supplying a callback
replaces the default named-font resolver, matching Rust's `Fonts::set_resolver`.
Returning `null` or `undefined` means unresolved and fails the render, even for
bundled ROM names. With no callback, bundled ROM lookup remains the default.

To retain ROM lookup explicitly, return `resolveRomFont(name)`. This helper
returns an opaque, reusable face handle for a known bundled font, `null` for
an unknown valid name, and throws for an invalid path. It is case-insensitive,
requires no disposal, and can also map a virtual filename to another ROM face.
Thrown errors fail the render at the selecting command. Promises are rejected;
load fonts before rendering if your storage API is asynchronous. No host files
are opened automatically, and font registrations never persist between renders.

In-job downloads take precedence over the resolver. External fonts support the
Rust engine's quadratic TrueType outlines and native hinting, with a 16 MiB
per-font ceiling. CFF/CFF2, variable fonts, and collections are unsupported.
Missing glyphs fail rather than falling back to a different face.
The callback resolves filenames used by `^A@`/`^CW`; it does not intercept
resident font-ID selections. Custom bitmap glyph-provider callbacks remain
unexposed; `resolveRomFont()` handles refer to the bundled ROM faces.

Inline `~DB`, `~DT`, `~DU`, and `~DY` font downloads use the Rust renderer's
existing support. The default **1 MiB input budget**, rather than a Wasm argument
size limit, can reject large downloads. Raise it explicitly for trusted jobs:

```js
const job = readFileSync('label-with-font.zpl');
const result = render(job, {
  limits: {
    inputBytes: 32 * 1024 * 1024,
    fontBytes: 16 * 1024 * 1024,
  },
});
```

`inputBytes` bounds both raw input and expanded stored formats separately.
`fontBytes` bounds total decoded inline downloads, including replacements and
bitmap allocation overhead; its default is 16 MiB. Both accept nonnegative
integers up to 4294967295. Hex downloads need roughly twice their decoded size
in the input budget. Use byte input for binary downloads. Other renderer/output
budgets and Wasm memory constraints still apply; raising these limits does not
disable them.

Invalid options, missing labels, unsupported commands, and resource-limit failures
throw errors. The Rust renderer's default input, label, geometry, and pixel
budgets apply. Rendering is synchronous; use Node worker threads for substantial
jobs in servers. Wasm result memory is freed internally; callers need no disposal.

This package exposes command framing, rendering, and output encoding. It does not
yet expose scene objects, bitmap glyph-provider callbacks, or individual
compatibility flags. See the repository's `docs/local-renderer.md`,
`docs/barcodes.md`, and `docs/printer-accuracy.md` for rendering coverage.
