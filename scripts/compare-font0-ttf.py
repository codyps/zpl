#!/usr/bin/env python3
"""Compare a supplied Font 0 TTF against captured ZBF strikes (no printer I/O).

Requires freetype-py. The font is supplied by the caller and is not copied.
FreeType load/render flags: https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html
ZBF coordinates are relative to the native FT baseline; never align glyphs to
hide placement differences. See docs/font0-ttf.md for provenance and limits.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import freetype


def decode(path):
    data = path.read_bytes()
    if data[:4] not in (b"ZBF1", b"ZBF2"):
        raise ValueError(f"Unsupported strike: {path}")
    height, width, dpi, count = struct.unpack_from("<HHHH", data, 5)
    offset = 13
    glyphs = []
    for _ in range(count):
        wide = data[:4] == b"ZBF2"
        codepoint = struct.unpack_from("<I" if wide else "<B", data, offset)[0]
        offset += 4 if wide else 1
        advance, left, top, w, h = struct.unpack_from("<HhhHH", data, offset)
        offset += 10
        length = (w * h + 7) // 8
        bits = data[offset:offset + length]
        offset += length
        points = {(left + x, top + y) for y in range(h) for x in range(w)
                  if bits[(y * w + x) // 8] & (128 >> ((y * w + x) % 8))}
        glyphs.append((codepoint, advance, points))
    if offset != len(data):
        raise ValueError(f"Trailing strike bytes: {path}")
    return height, width, dpi, glyphs


def compare(font, strike):
    height, width, dpi, glyphs = decode(strike)
    face = freetype.Face(str(font))
    face.set_pixel_sizes(width, height)
    rows = []
    for codepoint, advance, reference in glyphs:
        # Direct Unicode cmap, deliberately no printer-specific substitutions.
        face.load_char(chr(codepoint), freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO)
        glyph = face.glyph
        bitmap = glyph.bitmap
        buffer = bytes(bitmap.buffer)
        candidate = {(glyph.bitmap_left + x, -glyph.bitmap_top + y)
                     for y in range(bitmap.rows) for x in range(bitmap.width)
                     if buffer[y * bitmap.pitch + x // 8] & (128 >> (x % 8))}
        union = len(reference | candidate)
        rows.append(dict(codepoint=codepoint, glyph_index=face.get_char_index(codepoint),
                         underpaint=len(reference - candidate), overpaint=len(candidate - reference),
                         union=union, iou=(len(reference & candidate) / union if union else None),
                         printer_advance=advance, ttf_advance=round(glyph.advance.x / 64)))
    union = sum(row['union'] for row in rows)
    difference = sum(row['underpaint'] + row['overpaint'] for row in rows)
    return dict(strike=str(strike), sha256=hashlib.sha256(strike.read_bytes()).hexdigest(),
                height=height, width=width, dpi=dpi, glyphs=rows,
                foreground_iou=1 - difference / union if union else None,
                exact_glyphs=sum(row['underpaint'] == row['overpaint'] == 0 and
                                 row['printer_advance'] == row['ttf_advance'] for row in rows),
                below_80=[row['codepoint'] for row in rows if row['iou'] is not None and row['iou'] < .8])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path)
    parser.add_argument('strikes', type=Path, nargs='+')
    args = parser.parse_args()
    print(json.dumps(dict(font_sha256=hashlib.sha256(args.font.read_bytes()).hexdigest(),
                          freetype_version=freetype.version(),
                          results=[compare(args.font, strike) for strike in args.strikes]), indent=2))


if __name__ == '__main__':
    main()
