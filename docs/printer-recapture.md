# Capture the existing suites on the ZQ610 Plus

## Narrow paired controls and renderer profile

The sibling `zpl-comparison/benchmarks/zq610.py` prepares a focused 116-case
paired campaign, including narrow replacements for clipped content. Its gallery
is in `zpl-comparison/docs/benchmarks/zq610-plus/index.html`. It records missing
captures explicitly and compares local output on each native canvas.

`zpl::render::profiles::ZQ610_PLUS_203_DPI` defaults to the observed 384 × 2030
canvas and enables `preview_ignores_label_length`, 64-dot width rounding,
a 384-dot width cap, and width latching at the first draw. Each behavior has an
independent compatibility option. Set `Options.height` for a
different device configuration, or disable the flag to honor `^LL`. The default
renderer remains ZD621. The CLI accepts `--profile zq610-plus`.

The `zq610_preview` regression pins 120 native images, including four original
smoke captures with LL1218: 116 nonblank exact frames, three rotated Font 0
frames with precisely recorded residual differences shared by both printers,
and one deliberately blank clipping diagnostic. All 116 paired cases were
captured. Earlier HTTP stalls, explicit retries, and authorized restarts remain
in provenance. This focused corpus does not cover the entire historical
inventory. See the fixture README for precise scope.

`scripts/capture-printer-previews.py` inventories the checked-in printer PNGs in
both repositories and captures their source labels on a ZQ610 Plus. Python 3.9+
and Git are the only dependencies. Run from the `zpl` repository:

```sh
# Offline inventory; creates a new output directory.
python3 scripts/capture-printer-previews.py --output ../zq610-captures

# Capture all supported cases sequentially, or add --limit 3 for a smoke test.
python3 scripts/capture-printer-previews.py --output ../zq610-captures \
  --resume --capture --host http://zq610-plus.example.invalid/

# Offline integrity verification, independent of either source checkout.
python3 scripts/capture-printer-previews.py --output ../zq610-captures --verify
```

Use `--comparison PATH` if the comparison repository is not the sibling
`zpl-comparison`. Existing output requires `--resume`; a changed source inventory,
repository commit, printer identity or firmware requires a new output directory.
Successful, failed and interrupted submissions are never automatically replayed.
Failures stop the run and remain explicit; resume continues with unattempted cases.
Use a separate output directory for deliberate retries. Run only one preview
client against the printer, since the underlying temporary image is shared.

## Coverage and width policy

Font research is now a separate private checkout. Pass `--fonts /path/to/zpl-font-extract`
to include it explicitly; normal ZPL inventory does not require private repository access.

The inventory uses tracked PNGs in `zpl/tests/fixtures`,
`zebra-http-api/tests/fixtures`, the separate private font repository's
`zpl-font-extract/tests/fixtures`, and comparison's
`benchmarks/accuracy` and `references`. Generated website renderings and diff
images outside those roots are not printer inputs. Original duplicate paths and
historical diagnostic records remain separate cases. Source-less historical audit
images are resolved through the audit manifests, checking source and PNG hashes.
Other source-less images require an unambiguous same-stem source; ambiguity is
recorded, never silently guessed. Archived Labelary results are listed as excluded.

The inspected inventory contains 5,150 printer records: 5,106 replayable and 44
unsupported. Forty font calibration records require `~DY` downloads; four graphic
placement records require `~DG` and stored graphics. They stay in the plan with
their source/reference hashes and reasons, and are not submitted. These counts
will evolve with the repositories. All cases with an existing captured PNG are
inventoried; source-only failures without a PNG are not a captured baseline.

The default width is **384 printer dots**, measured on ZQ610 Plus 203 dpi,
firmware V100.21.21Z. Every explicit `^PW` is clamped to this maximum, retaining
deliberately narrower widths. Labels without `^PW` receive the target width.
Field origins, text, fonts, barcode module sizes and graphic bytes remain intact.
This is a narrower canvas: content beyond its edge can be clipped. It is not a
scaled or reflowed layout, and cannot establish coverage of fields clipped away.
Field/comment payloads and counted GFB bytes are preserved when changing width.
Unsupported command/framing forms are recorded rather than edited heuristically.

Each submission starts with the original capture height and the comparison
harness's clean layout reset, including identity mappings for CI0/CI13 and
`^BY2,3,10`. Source commands can override that reset. This standardizes the new
campaign; it does not replay every historical contaminated/default-state setup.
The exact reset and submitted bytes are saved. See the sibling
`references/preview-state-audit-20260921/README.md` for the reset evidence and
`zebra-http-api/src/lib.rs` for the HTTP protocol.

The live smoke test returned **384 × 2030**, despite shorter requested heights.
Returned PNGs are preserved unmodified. Per-case dimensions and canvas-match
observations expose this behavior; a successful download does not establish
rendering accuracy, full visibility, or a nonblank result. The PNG validator
checks framing and CRCs, not decoded raster correctness.

## Provenance

`plan.json` records repository commits and working-tree status, all case IDs,
source and original PNG paths/hashes/dimensions, original widths, submission
hashes, unsupported cases and exclusions. `provenance.json` records:

- Printer URL, model, serial (reported BT friendly name), firmware and DPI.
- UTC campaign/session/request timestamps, tool SHA-256, and the plan SHA-256.
- SHA-256 for every saved source, exact submission, returned HTML, PNG, status
  page and reset file. Raw status pages preserve media and other reported state.
- Per-case status, errors, returned image URL, and native PNG dimensions.
- Start/end control captures per session and their PNG byte equality. Byte
  differences require review; they are not assumed to prove pixel differences.

The manifest is checkpointed before each POST and after each result. Requests use
bounded timeouts and responses; redirects and image fetches remain on the printer
origin. Only the `prev=Preview Label` form operation is sent. No Print operation,
font download, firmware upload or persistent configuration save is performed.
Preview layout commands can affect in-memory rendering state.

The overall status remains `incomplete` while any case is unsupported, unresolved,
unattempted, failed, or a session lacks equal repeated control bytes. In particular,
this inventory cannot claim full completion while its 44 download-dependent
records remain unsupported. Existing ZD621 baselines are never overwritten or
automatically installed as ZQ610 test expectations.

Run focused offline tests with:

```sh
python3 -m unittest discover -s scripts -p 'test_capture_printer_previews.py'
```
