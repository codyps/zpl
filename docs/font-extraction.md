# Extracting preview fonts

The Rust `extract-font` example samples resident glyphs through
`zebra_http_api::zpl_to_png_with_credentials`. Sampling, PNG decoding, baseline
measurement and BDF generation live in `zpl::font_extract` and `zpl::output`.
It reuses libraries already in the workspace; no new external crates were added.
Python and imaging executables are not needed. Only preview requests are sent.

## Run

```sh
direnv exec . cargo run -p zebra-http-api --example extract-font -- \
  --host http://printer.local/ \
  --font 0 --height 32 --width 0 --dpi 203 \
  --verify-text 'AVATAR Agj Wavy 123 _^~|!' \
  _font-0-32
```

The default captures all 95 printable ASCII characters in 12 sequential preview
requests. `--characters ' AgjW'` selects a smaller set. `--font` accepts `0` and
`A`–`H`; repeat the command into another directory for another font or size.
Width zero asks the printer to choose its default width. DPI is metadata for BDF
export; supply the printer's actual resolution.

If form authentication is needed, set `ZPL_USERNAME` and `ZPL_PASSWORD` in the
process environment. Credentials are not stored in the output. URLs with embedded
credentials are rejected. Defaults match the existing client's empty form fields.

Output directories must be new unless `--resume` or `--offline` is given:

```sh
# Reuse matching pages and fetch only missing ones.
direnv exec . cargo run -p zebra-http-api --example extract-font -- --host http://printer.local/ --resume _font-0-32

# Re-extract and repeat a previously captured verification without network access.
direnv exec . cargo run -p zebra-http-api --example extract-font -- --host http://printer.local/ --offline \
  --verify-text 'AVATAR Agj Wavy 123 _^~|!' _font-0-32
```

Use the same font, dimensions, character set and batch size when resuming.
Existing captures from the former Python tool are compatible; both versions use
the same configuration, page ZPL and JSON/BDF formats.
`--batch-size`, `--delay`, and `--timeout` control page size and request pacing.
Run against an idle preview endpoint: concurrent preview clients can replace the
shared `TEST1` preview object used by the existing API.

## Outputs and measurement

- `capture.json`: sampling configuration and source URL.
- `page-NNN.zpl` and `.png`: reproducible requests and unmodified responses.
- `font.json`: glyph bitmaps, advance widths, bearings, baseline offsets and
  SHA-256 hashes of source pages.
- `font.zbf`: compact bit-packed strike for embedding (see [format](../zpl/assets/README.md)).
- `font.bdf`: the same glyphs in [BDF 2.1](https://www.x.org/releases/X11R7.0/doc/PDF/bdf.pdf).
- `verification.*`: optional held-out text request, PNG, and comparison report.
- `verification-diff.png`: colored comparison (magenta = extracted glyphs only;
  cyan = printer preview only; dark gray = shared ink).

Each tile contains an isolated glyph positioned with `^FT` and a `|glyph|` probe.
The difference between the right edges of `|glyph|` and a reference `||` gives
advance width, including the advance of a blank space. The trailing sentinel's
bitmap is checked. Field hex escapes safely encode `^`, `~`, and `_`.

JSON coordinates are printer dots: `left` and `top` are relative to the baseline
origin, with positive Y pointing down. `advance` moves the next glyph's origin.
`bitmap` contains top-to-bottom hexadecimal rows, most significant bit first,
padded with zero bits on the right. Empty space has no bitmap and a nonzero
advance. To draw through the local renderer's path model, turn each black pixel
(or horizontal run) into a rectangle at `(pen_x + left + x, baseline + top + y)`;
advance `pen_x` after each glyph. The renderer now embeds the complete font 0 strike at 32 dots from `font.zbf`.
See [embedded font](../zpl/assets/README.md) for regeneration and integration.

`--verify-text` composes these glyphs independently and compares them pixel for
pixel against another printer preview. A mismatch fails the command and leaves
a report; exports are replaced only after successful verification. A match proves
that sampled size and string, not every possible font context or scale.

## Boundaries and validation

This extracts **raster strikes at specific requested sizes**, not original font
files, outlines, hinting instructions, or complete Unicode fonts. Other sizes
need separate captures. Contextual shaping or kerning may invalidate independent
advances; use held-out verification text representative of your labels.

PNG decoding validates CRCs, filters, format and decompressed length. It supports
noninterlaced 1/2/4/8-bit grayscale/indexed images and 8-bit RGB/alpha images.
Unsupported formats, clipped glyphs, empty probes and scaled/cropped pages fail.
Canvases align to 64-dot widths, as observed on the ZD621 preview. Large requested
sizes may exceed a printer's canvas; reduce height/width or batch size if needed.

Validation on the available ZD621 at 203 DPI captured all 95 printable ASCII
glyphs for font 0, requested height 32 and width 0. The held-out string
`AVATAR Agj Wavy 123 _^~|!` matched exactly (zero differing pixels). Captures live
in the ignored `_font-zd621-0-32/` directory; firmware-specific assets are not
added to the source tree automatically.

The Rust port replayed all 95 existing glyphs with identical JSON values and
byte-identical BDF output. A fresh nine-glyph live capture and its held-out text
also matched exactly. Tests cover decoding, metrics, baselines, escapes,
configuration matching, offline replay, failure preservation, SHA-256 vectors,
and HTTP transport boundaries.

```sh
direnv exec . cargo test -p zpl --lib
direnv exec . cargo test -p zebra-http-api --lib --example extract-font
```

## Size and rotation study

See the [font reconstruction study](font-reconstruction.md) for live captures across 14 size
configurations, all four rotations, larger-strike scaling, and the assessment
of outline and hint-parameter fitting.
