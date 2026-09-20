# Aligned-width barcode conformance captures

These 60 raw ZD621 203-DPI V93.21.33Z HTTP previews were captured on
2026-09-20, serially with five-second pacing. Every request completed.
The original comparison corpus requested PW812 but the device returned 832
pixels. These new inputs explicitly request PW832 and embed the original
capture reset inside the label. Per-case JSON records both original and new
source hashes, reset commands and the native PNG hash. No PNG was resized,
padded or registered. The sibling comparison repository is unchanged.

The printer-profile regression pins 59 rendered cases: 58 match exactly;
`symbol-qr` retains 464 underpaint and 496 overpaint pixels (77.01% foreground
IoU). Its automatic mask-selection discrepancy remains unresolved; pinning
these counts does not claim that the non-text accuracy goal is met.

`symbol-databar_upce` supplies six compressed digits to BR variant 8, while
our ZD621 profile requires eleven uncompressed UPC-A digits. The device
returns an entirely blank page and the renderer rejects the input. Its raw
capture is retained as invalid-input evidence, not scored as a successful
barcode. It does not establish that a blank page is correct for valid UPC-E.

Source: complete, hash-verified conformance reference from
`zpl-comparison/benchmarks/accuracy/conformance-reference`. New input changes
are only PW812 to PW832 and insertion of the recorded reset. All original
source hashes are retained in the per-case JSON files.

Command reference: Zebra [ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
barcode command definitions pp. 73–143, BR p. 135, PW p. 329. Compatibility
options use the explicit ZD621_203_DPI profile.
