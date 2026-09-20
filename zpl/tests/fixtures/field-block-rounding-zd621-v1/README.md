# Justified word positions

Six raw ZD621 203-DPI V93.21.33Z previews captured 2026-09-20. Each frame has
12 field blocks, for 72 independent controls. Fonts 0 (24 by 24) and A (9 by 5)
cover one, two and three word gaps. Varying block widths distribute fractional
slack between words; a short final word forces wrapping and verifies the final
line stays left-aligned. Every complete canvas must match exactly, without
alignment, cropping or scaling. Input, capture and rendered-pixel SHA-256 hashes
and zero underpaint/overpaint are pinned in the manifest.

The printer rounds each justified word position upward, rather than to the
nearest dot. Two-gap half-dot positions distinguish flooring; three-gap thirds
distinguish nearest rounding. Position calculations distribute cumulative slack
directly, avoiding floating-point accumulation across gaps. This fixes the
independent `GS-blocks` control from 55 underpaint/overpaint dots to zero.

`block_justification_rounds_up` is enabled in ZD621_203_DPI and disabled in
SPECIFICATION. This is an observed printer dot-quantization choice; the guide
specifies word justification without prescribing fractional-dot rounding.

Source: [Zebra ZPL II Programming Guide](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf),
^FB pp. 185–187. This suite uses short words; automatic hyphenation and soft
hyphen handling require separate coverage.
