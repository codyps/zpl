# DataBar retail controls

Captured 2026-09-19 UTC using HTTP Preview Label on
`http://printer.local/`, ZD621 203 dpi, firmware V93.21.33Z.
No physical labels were printed. PNG files are original printer response bytes.
The manifest pins source and response SHA-256. Tests use the printer profile,
compare the complete canvas without shifting, cropping or rescaling, and require
nonblank exact equality for valid inputs. Invalid inputs require renderer errors,
not successful blank barcode renders.

Six UPC-A/EAN-13/EAN-8 controls vary module width (1/3) with height operand 20;
the original corpus uses module width 2 and height 80. The printer ignores that
operand and gives all bars the same height: 74X (EAN-8: 60X), with 7X left margin.
These dimensions are a compatibility option. Specification behavior uses GS1
nominal height/X ratios and five-module guard extensions. References: Zebra ZPL
Programming Guide ^BR p. 135; [GS1 General Specifications 23.0, EAN/UPC symbol
dimensions](https://ref.gs1.org/standards/genspecs/23.0.0/).

Four valid UPC-E controls cover each zero-suppression form of ISO/IEC 15420.
The printer requires eleven uncompressed UPC-A digits for BR8, unlike standalone
B9. Four compressed or check-digit-inclusive controls produce blank responses.
The fifth invalid control preserves the original corpus input/response unchanged.
The original positive case was corrected to 04210000526 and recaptured separately
at its original 832x1218 canvas. This is not counted as parity with blank output.
