# ZD621 DataBar Expanded compaction controls

48 raw HTTP Preview Label captures from Zebra ZD621 203 DPI, firmware
V93.21.33Z, collected 2026-09-18. Each input resets label state and uses a
832-dot width to avoid preview width adjustment. Captured with the repository's
`zebra-render` example. No images are cropped, rescaled or repaired.

The controls cover checked GTINs, supplementary data, short metric/imperial
weights and their boundaries, all eight weight/date methods, prices with and
without currency, and stacked/unstacked layouts. Source: ISO/IEC 24724:2011
§7.2.5.4, pp. 26–29, Table 10; supplied BSI edition, ISBN 9780580626906.

`manifest.tsv` records input/PNG SHA-256, separate underpaint/overpaint counts,
and the local decoded pixel SHA-256. Tests use `ZD621_203_DPI`. Zero differences
are required where achieved; remaining differences are pinned, including exact
pixel positions, so an equal-size but different error cannot pass unnoticed.

All 48 controls are pixel exact. The profile option
`databar_expanded_wide_bar_separator` reproduces the observed A1/B1 separator
templates next to four-module data bars. Nineteen B1 observations leave a
four-module space solid; two independent A1 observations also move ink beneath
a bar. The other controls verify the ordinary templates. SPECIFICATION retains
the standard's light dots beneath bars and alternating dots beneath spaces.
These empirical templates describe firmware V93.21.33Z; they are not a change
to the DataBar specification.

The long-weight
no-date printer controls contain nonstandard headers and repeat the final input
digit after the 38400 no-date sentinel. The ZD621 profile reproduces these
headers and data; all four unstacked long-weight controls are pixel exact.
Disable `databar_expanded_no_date_preview` to retain the standard's complete
fixed-length encoding. Independent decoder tests cover the standard outputs.
The original two comparison-corpus Expanded cases are both pixel-exact.
