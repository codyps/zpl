# Original constructed-font campaign, 2026-10-02

See [results and interpretation](../../../../docs/font-probe-results-20261002.md).
These are development fixtures for the downloaded TrueType path, not production
font assets or sealed holdouts. Exact printer identities, firmware, timestamps,
and request hashes are in each device's `capture.json`.

`probe.ttf` is an original 3,540-byte, 59-glyph SFNT (including `.notdef`), built
from the contours and instructions in `manifest.json`. SHA-256:
`6366abca64676b4b950cb24fa607c5b29ff5714daf46f3276f1c522ffca07dd9`.
The generator reuses ancillary tables from the earlier original
`zebra-http-api/tests/fixtures/font-refinement-v1/calibration-plain.ttf`, verified
against SHA-256 `8d170e65a6109ef3dd16c30ef683809fae81342ccf59d496a9f210cfd4863e19`.
No third-party font outlines are included.

- Root ZPL files: the 19 prepared pages with explicit encoding and geometry.
- `zd621/`: 21 previews on ZD621 D7J211001302, 203 DPI, V93.21.33Z. The final
  status remains `cleanup-failed` because the original HTTP delete was ineffective.
  `cleanup-recovery.json` links that record and the exact font/device to confirmed
  removal by SGD. The analysis validates both records instead of rewriting history.
- `zq610/`: 21 previews on ZQ610 Plus XXZMJ230802993, 203 DPI, V100.21.21Z.
  Submitted requests use `^LLh,Y`. Both affected settings were restored and checked.
- `zq610-height-diagnostic/`: initial 384×2030 response to a 384×192 resident
  control; no font was uploaded during this failed attempt. Retained as an
  observation, excluded from successful native-canvas comparisons.
- `analysis.json`: FreeType 2.13.2 comparisons, sentinel metrics, known-transform
  rotation diagnostics, equivalent-curve pairs, and cross-device native-page XOR.

Resident controls repeat exactly within each completed run. Both runs verified
font selection and instruction execution, and removed only the owned RAM object
`R:ZP26A.TTF`. Request journals reserve each preview before sending it. There
were 43 preview requests total, including the diagnostic failure, with five-second
pacing; no formats were sent to the physical print path.

From the repository root, reproduce offline (no printer access):

```sh
python zpl-font-extract/scripts/font_probe.py /tmp/font-probe-prepared
uv run --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python zpl-font-extract/scripts/analyze_font_probe.py \
  zpl-font-extract/tests/fixtures/font-probes-20261002 \
  --output /tmp/font-probe-analysis.json
uv run --no-project --with freetype-py==2.5.1 --with pillow==12.3.0 \
  python -m unittest discover -s zpl-font-extract/scripts -p 'test_*.py' -v
```

Outputs must not already exist. The Python bindings do not pin the underlying
FreeType shared library; the analysis records its version and the baseline test
requires 2.13.2. Use a separate report when evaluating another version. Tests also
check deterministic fixture generation, SFNT checksums, strict execution of all
373 isolated glyph probes, independent X/Y witnesses, phantom-point advances,
and exact rational equality of the quadratic variants.

The device-changing capture commands used these arguments (choose fresh output
paths if repeating them; do not overwrite these fixtures):

```sh
uv run --no-project --with pillow==12.3.0 \
  python zpl-font-extract/scripts/capture_font_probe.py \
  zpl-font-extract/tests/fixtures/font-probes-20261002 \
  --output /tmp/font-probe-zd621 \
  --host d7j211001302.bed.einic.org --model ZD621 --serial D7J211001302
uv run --no-project --with pillow==12.3.0 \
  python zpl-font-extract/scripts/capture_font_probe.py \
  zpl-font-extract/tests/fixtures/font-probes-20261002 \
  --output /tmp/font-probe-zq610 \
  --host xxzmj230802993.bed.einic.org --model 'ZQ610 Plus' \
  --serial XXZMJ230802993 --force-label-length
```

The capture script was corrected during the campaign: failed previews now retain
returned dimensions/hash before rejecting them, deletion uses SGD, and the optional
label-length override saves/restores both affected settings. Older records retain
their original schema fields and failures. The scripts are bounded research tools
for these prepared manifests, not a font-upload interface for the public proxy.

## Original engine reference

`freetype-outlines.json` contains 464 independent hinted-outline/advance cases
from this original font at eight X/Y sizes, generated with FreeType 2.13.2, native
monochrome hinting, no autohinter, and pedantic error checks. Reproduce it with:

```sh
uv run --no-project --with freetype-py==2.5.1 python \
  zpl-font-extract/scripts/compare_truetype.py \
  zpl-font-extract/tests/fixtures/font-probes-20261002/probe.ttf \
  --reference-output /tmp/freetype-outlines.json
```

`zpl/tests/truetype.rs` checks these outlines and the original full native printer
canvases. It preserves directional pixel residuals and hashes; experimental scan
conversion is not asserted to be printer-exact. See
[implementation results](../../../../docs/font-scaling-options.md#original-engine-implementation).
