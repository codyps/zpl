# Barcode mode controls

Captured 2026-09-18–19 (UTC) from the HTTP Preview Label endpoint of
`http://printer.local/`, ZD621 203 dpi, identified firmware
V93.21.33Z. Only preview requests were sent; no labels were printed.

Sources use PW832 with LL400 or LL1218. Layout, module sizes, orientations,
origins and validation switches are explicit in each `.zpl` file.
Each PNG is the original printer response. `manifest.tsv` pins source/image
SHA-256, current renderer pixel SHA-256, and separate underpaint/overpaint counts
using the ZD621 profile, threshold 128, identical coordinates and no padding.

- CODABLOCK A: columns 4/6/10, explicit/automatic rows, security on/off,
  short data, padding and long checksum sums. All nonblank controls are exact.
  Overcapacity probes returned blank printer images and are covered as renderer
  errors separately rather than accepted as valid barcodes.
- Code 128: automatic numeric/mixed data, explicit subsets A/B/C, switching,
  FNC1, UCC case mode and parenthesized GS1 data. All controls are exact.
- QR Model 1: all four error levels and all versions 1–14, including multi-block
  RS and remainder words. Firmware selects its own mask despite requested mask
  0. The ordinary profile render's differences are pinned, not suppressed.
  A separate encoding test reads the actual mask from the immutable reference
  and verifies exact modules with that mask. This does not assert automatic-mask
  parity. rxing 0.9.3's generic QR data-block deinterleaver cannot validate Model 1
  multi-block symbols; single-block symbols also have independent decoder tests.

Command/encoding references: Zebra guide ^BC pp. 94–103, ^BB pp. 90–93;
ISO/IEC 18004:2000 Annex M (Model 1 bitstream, RS tables and module placement).
The implementation is original; no third-party encoder source was used.

- Legacy Data Matrix: 56 controls cover ECC 000/050/080/100/140, all six
  alphabets, every odd symbol size 9–49, and binary payloads through `^FH`.
  All are pixel-exact, including the largest 49×49 symbol.
- Multiple origins: 31 controls cover PDF417 and MicroPDF417, one/multiple
  segments, numeric/lowercase data, all orientations, excluded origins, and
  too few origins. Four inputs reproduce the comparison conformance cases.
  All are exact; insufficient origins intentionally produce blank labels.
- Validation: eight controls cover valid data, C/E/S/P labels, and printer
  departures for retail overlength and forced legacy Data Matrix dimensions.
  The small validation alphabet in `validation.rs` was sampled from these
  original PNGs, including L from the word INVALID. All eight are exact.

The 159 controls include 138 exact ordinary renders and 21 QR mask differences.
Legacy Data Matrix follows ISO/IEC 16022:2000 §§5.2–5.7 and Annexes B–F.
The compact placement permutation was checked against the published grids and
all 21 supported printer symbol sizes; no grid implementation was imported.
Macro PDF417 follows USS PDF417 Appendix G; `^FM` follows Zebra pp. 197–199,
and `^CV` follows p. 167. Macro file-ID codewords [0,0,36], MicroPDF417 reverse-origin shifts, and the two validation-code departures are explicit
printer profile options.

The validation tests do not claim to reproduce every firmware error path:
empty Code 128, raw lowercase Code 39, and undersized ordinary PDF417 probes
had additional firmware behavior not emulated here (including an Internal Error
panel). These exploratory captures were not substituted for the accepted
controls. Unsupported encoder modes still return renderer errors under `^CVY`.

Three CV responses initially included stale bytes after the PNG IEND chunk.
They were recaptured using distinct preview object names CVREG0/1/2 instead of
TEST1. The replacement responses have complete PNG framing and identical decoded
pixels; their original response bytes and new SHA-256 values are retained here.
