# ZD621 MaxiCode preview controls

61 unmodified HTTP Preview Label captures from Zebra ZD621, 203 DPI,
firmware V93.21.33Z, collected 2026-09-18 with `zebra-render`.
Each input resets label state and uses PW832 to avoid width adjustment.
The manifest pins ZPL and PNG SHA-256 and the number of dark pixels. Tests
require zero overpaint and underpaint over the entire canvas with ZD621_203_DPI.

The suite covers modes 2–6, A/B run lengths, C/D/E shift/lock-in boundaries,
nine-digit compression, common punctuation, NUL, minimum input length and FO/FT.
Sources: Zebra Programming Guide ^BD pp. 106–108; ISO/IEC 16023:2000 Annex A
pp. 24–25 and Annex F.1–F.4 pp. 32–33 (supplied BSI edition, ISBN 9780580364105).
No standards document or third-party encoder source is copied into the code.

## Printer departures

- A fixed 6×7-dot hexagon, 7×6 pitch, three-dot odd-row offset and concentric
  finder dot pattern replace nominal continuous geometry. Module and finder
  templates are independent of data. FT subtracts the full 199-dot symbol height.
- A final codeword 63 precedes padding, including in set A/B.
- NUL terminates the data string. The standard supports NUL in set E.
- Mode 4/6 inputs shorter than six bytes produce blank previews. These seven
  blank controls are negative controls; passing them is not an encoding success.
- All six mode-5 controls show only fixed finder/orientation marks. These are
  deliberately undecodable firmware previews. The mode-5 omission option is
  independent of dot geometry; disable it for complete mode-5 barcode data.

Each departure has an independent Compatibility option, enabled by ZD621_203_DPI
and disabled by SPECIFICATION. Decoder tests use specification behavior and
independently decode every byte value, including C/D/E runs and complete mode 5.
