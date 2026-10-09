# C API

`zpl-c` provides a C ABI for the `zpl` parser, renderer, and output adapters.
It builds `libzpl_c.so` (Linux), `libzpl_c.dylib` (macOS), or `zpl_c.dll`
(Windows), plus a static library. It adds no runtime dependencies to `zpl`.

```sh
cargo build --locked --release -p zpl-c
cc -std=c11 -Wall -Wextra -Werror -Izpl-c/include zpl-c/examples/render.c \
  -Ltarget/release -lzpl_c -Wl,-rpath,"$PWD/target/release" -o /tmp/zpl-render-c
/tmp/zpl-render-c
```

The example writes `label.png` in the current directory. Adjust the library
path if using `CARGO_TARGET_DIR`. On Windows, link the generated import library
and put the DLL beside the executable. For static linking, obtain required system
libraries with `cargo rustc --locked --release -p zpl-c -- --print native-static-libs`.
Distribute both `include/zpl.h` and `include/zpl_config.h` alongside the library.
The header supports C11 and C++; no Rust toolchain is needed by consumers of the
built artifacts. ABI version 1 requires matching headers/library releases; struct
layouts may change in a future ABI version. Check `zpl_abi_version()` at startup.

The API exposes:

- Lossless complete-stream parsing, byte offsets, element kinds, initial/final
  syntax, unknown commands, and binary payloads.
- All three renderer profiles, dimensions/DPI, every independent compatibility
  setting, and all render/output resource budgets.
- Bitmap font providers and copied TrueType fonts, registered by ID or virtual filename.
- Multiple labels, warning text, scene dimensions, PNG, SVG, single/multipage PDF,
  and row-major grayscale pixels.
- Typed status codes, original error text and byte offsets, and thread-local
  errors. Rust unwinding panics are contained at fallible ABI entry points.

Initialize configuration structs with their `*_init` function before overriding
fields. NULL options use the ZD621 profile, matching `zpl::Options::default()`;
select `ZPL_PROFILE_SPECIFICATION` for strict specification behavior. NULL limits
use the native bounded defaults. All numeric limits must be finite and
nonnegative. Optional fields use an explicit `present` flag; all boolean and
presence flags must be 0 or 1. See the [native compatibility documentation](../zpl/src/render/compatibility.rs)
for individual settings, and [renderer coverage](../docs/local-renderer.md) for
supported commands and font behavior.

Inputs are byte slices, not C strings. Returned bytes are length-delimited and
not NUL-terminated. Each document, scene, parsed result, and encoded buffer is
owned by its caller and must be freed exactly once with the corresponding library
function. Scenes and encoded buffers survive their source document. Warning and
parsed-element byte views are borrowed from their containing handle; buffer views
are borrowed from their buffer. Error text is borrowed until the next
status-returning call on the same thread. Copy it first if it must outlive that
call. See `zpl.h` for nullability, pointer validity, and concurrency contracts.

This interface covers the parser/render/output workflow. Rust scene construction,
standalone bitmap/TrueType inspection, filename resolver callbacks, and caller-owned
raster targets are not exposed. Fonts downloaded inside ZPL retain native behavior.
No printer transport, admission policy, or network access is added.

## Custom fonts

Create a `ZplFonts` collection with `zpl_fonts_new`. Register bitmap callbacks with
`zpl_fonts_insert_bitmap` (ID) or `zpl_fonts_insert_named_bitmap` (virtual filename),
then pass the collection to `zpl_render_with_fonts`. The callback receives a Unicode
codepoint and returns a glyph, a missing result, or an error message. Glyph bitmaps
are contiguous MSB-first rows with `ceil(width/8)` bytes per row. Metrics match
[`zpl::fonts::BitmapFont`](../zpl/src/fonts/mod.rs): cell dimensions, a dot baseline,
pen advance, and offsets from that baseline.

```c
static const uint8_t rows[] = {0x40, 0xa0, 0xe0};
static int32_t lookup(void *context, uint32_t codepoint,
                      ZplGlyph *out, ZplBytes *error) {
    (void)context;
    (void)error;
    if (codepoint != 'A') return ZPL_GLYPH_MISSING;
    *out = (ZplGlyph){8, 0, -3, 3, 3, {rows, sizeof(rows)}};
    return ZPL_GLYPH_FOUND;
}
/* Inside the caller; check each status before proceeding. */
ZplFonts *fonts = NULL;
ZplBitmapProvider provider = {{8, 12, 9.0}, NULL, lookup};
int32_t status = zpl_fonts_new(&fonts);
if (status == ZPL_OK)
    status = zpl_fonts_insert_bitmap(fonts, 'Z', &provider);
ZplDocument *document = NULL;
const uint8_t source[] = "^XA^FO20,20^AZN,12,8^FDA^FS^XZ";
if (status == ZPL_OK)
    status = zpl_render_with_fonts(source, sizeof(source)-1, NULL, fonts, NULL, &document);
zpl_fonts_free(fonts);
/* document is independent of fonts; encode it, then free it. */
zpl_document_free(document);
```

The library copies the provider descriptor and each returned glyph. Callback
context remains caller-owned and must live until all registrations using it are
replaced or freed. Returned byte storage must remain valid and immutable until
the render returns. Callbacks must be deterministic, support concurrent renders,
and return normally without exceptions or `longjmp`. Do not mutate or free the
font collection during rendering. A missing glyph fails rendering; it does not
fall back to a resident font. Callback errors retain the selecting field's ZPL
byte offset in the render diagnostic.

`zpl_fonts_insert_truetype` and `zpl_fonts_insert_named_truetype` copy supplied
font bytes, so input storage may be released immediately after registration.
Use `ZPL_HINTING_NONE` or `ZPL_HINTING_NATIVE`. Format and hinting support match
the native quadratic-outline TrueType engine; registration does not read files.
Named registrations support `^CW` and `^A@`, normalize case and the default `R:`
device, and replace prior resources under the same name. Failed registration
preserves the previous assignment. Free the collection with `zpl_fonts_free`;
rendered documents and their encoded output remain independent of it.

See [the compiled C/C++ font test](tests/fonts.c) for a complete ownership example
using both bitmap callbacks and TrueType data.

## Validation and maintenance

```sh
python3 zpl-c/generate-config.py --check
cargo test --locked -p zpl-c
sh zpl-c/tests/run-c.sh
```

The C test builds and links the actual shared library, exercises ownership and
failure paths, and compiles the header as C++. Rust tests compare every output
format directly with native adapters. `generate-config.py` uses only Python's
standard library; rerun it when native configuration fields change, and review
whether the ABI version must change. The generator checks all native fields and
fails on unsupported types; it is not required to build the library.
