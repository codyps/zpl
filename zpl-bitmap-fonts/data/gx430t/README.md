# GX430t fixed scalable strikes

This rendering dataset contains native GX430t 300-DPI (12 dots/mm), firmware
V56.17.17Z, serial 32J193600198 preview observations. It is selected independently
of DPI by `captures::gx430t`; the default/ZD621 dataset remains unchanged.

`captures.json` normalizes 43 base strikes (36 font-0 sizes and P through V),
36 legacy backslash/cent supplements and two PA1 supplements: 81 captures,
4,136 pooled records, 120 compiled maps and 221,285 bitmap bytes. Base and
supplement ordering remains explicit. An absent glyph/size is not a measured
blank. Unsampled strikes still use the documented fallback approximation.

Acquisition PNGs, exact submitted ZPL and original HTML responses are in
[`zpl/tests/fixtures/gx430t-300-v1`](../../../zpl/tests/fixtures/gx430t-300-v1/README.md),
including independent sizes, Unicode, rotations and held-out text. Raw source
capture digests remain in the normalized JSON. Legacy remapping was reset before
reacquiring affected barcode captions. Printer identity, state failures and
scope limits are documented with those fixtures. These observations make no
claim about other firmware or universal glyph recovery.

Regenerate the fixed rendering assets with the dependency-free compiler:

```sh
python zpl-bitmap-fonts/data/compile_captures.py --dataset gx430t
python zpl-bitmap-fonts/data/compile_captures.py --dataset gx430t --check
```

No fitting, sampling or extraction implementation is included here. Continuing
font research belongs in the separate private zpl-font-extract repository;
this workspace has no dependency on it. Font content retains its original
licensing; repository code uses OSL-3.0.
