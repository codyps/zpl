# ZPL reference and parser design

[ZPL library comparison](https://github.com/codyps/zpl-comparison) contains the
standalone benchmarks, printer accuracy reports, rendering conformance corpus,
and compatibility reference by library, command and feature.

[Proxy telemetry](telemetry.md) documents fastrace spans, correlated logs, and
optional OTLP export.

[Printer proxy cache](proxy-cache.md) documents persistent ZPL, render results,
request history, and cache refresh controls.

[Browser preview](web-preview.md) documents the local WebAssembly editor and
GitHub Pages deployment.

[Renderer HTTP API](worker-api.md) documents the public Cloudflare Worker,
Labelary compatibility, rate limits, hosting comparison, and deployment.

[Release PR automation](releases.md) documents release-plz setup and the separate
crates.io publishing prerequisites.

## Vendor specification

[Zebra ZPL II, ZBI 2, Set-Get-Do, Mirror, WML Programming Guide](zpl-zbi2-pg-en.pdf)

- Publisher: Zebra Technologies.
- Revision: **P1134473-11EN Rev A**, dated **2026-07-05**.
- Retrieved: **2026-09-14** from the [official PDF](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf).
- SHA-256: `b1f83b0822f176bb20b7cfe14a37ea33fb552c3d6bcf05da1b4c2704ad3aaa0c`.

[Parser coverage and API](parser-coverage.md) explains what the command-stream parser does.
[Implementation comparison](parser-comparison.md) records source-level comparisons with other projects.
[Command index](zpl-command-index.tsv) lists 223 spellings from the standard, network, and RFID command sections, plus `~DL` mentioned in the encoding appendix.

## Relevant sections examined

| Topic | PDF pages |
| --- | --- |
| ZPL / other printer-language separation | 47 |
| Standard ZPL command reference | 48–376 |
| Prefix and delimiter changes | 152–153, 166 |
| Bitmap, encoding, graphics, and font downloads | 170–184 |
| Field text and hexadecimal escapes | 190, 193 |
| Single-byte field separator and label delimiters | 204, 370, 375 |
| Comments and graphic-field framing | 209, 215–216 |
| Network commands | 377–411 |
| RFID commands | 412–441 |
| B64 / Z64 framing and per-glyph downloads | 1602–1605 |
| Syntax configuration and abbreviated operands | 1758–1759, 1765–1767 |

The `~DY` entry's format line on page 181 says `~DV`, while its heading, examples, command index, and surrounding text say `~DY`. The parser implements `~DY`; it does not invent a binary `~DV` alias from that inconsistent line. `~DL` has an appendix mention but no complete syntax entry in this revision; it is retained as an opaque command with encoded subfield recognition.

Regenerate the index after checking a replacement guide's layout:

```sh
python3 scripts/index-zpl-reference.py
```

This requires `pdftotext`. The generator intentionally checks the pinned revision's command count so a changed manual cannot silently shrink the coverage set.

## Local rendering

See [Local renderer](local-renderer.md) for the path-based intermediate output,
PNG/SVG adapters, supported command subset, limitations, and CLI examples.

## Font sampling

[Font extraction](font-extraction.md) documents the Rust preview API
sampler, bitmap font exports, resumable captures, and pixel-level verification.

## Rendering comparisons

[Binary PNG image diff](raster-diff.md) describes the Rust `png-diff` tool,
its directional colors, pixel statistics, and CI exit codes.

[Font reconstruction study](font-reconstruction.md) measures size and rotation
accuracy and assesses recovery of scalable outlines and rasterization rules.

[Font parameter fitting](font-fitting.md) describes the offline Rust optimizer,
fitted outlines, and training-versus-validation results.

[Stroke-width parameter fitting](font-stroke-fitting.md) extends the outline
experiment with size-dependent quantization and reports held-out accuracy.

[Font refinement plan](font-refinement-plan.md) prioritizes pipeline calibration,
TrueType parameter recovery, fresh validation, and structured hint fitting.

[Font refinement execution](font-refinement-results.md) records the new capture
campaign, calibration results, recovered spacing constraints, and the gate that
keeps the experimental model out of the renderer.
