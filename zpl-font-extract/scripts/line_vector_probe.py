"""Original SFVTL/SHPIX control font and independent FreeType outline evidence.

The control isolates a constant displacement normal to a diagonal stroke;
it does not fit resident-font observations. Stack order and freedom direction:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#sfvtl
"""

import argparse
from pathlib import Path

import freetype

import diagonal_font_hints
import font_probe
from reconstruct_font import save, sha


def prepare(root):
    shape = [[(0, 0, True), (1000, 1000, True), (1150, 850, True), (150, -150, True)]]
    items = [
        dict(
            name=name,
            codepoint=ord(char),
            contours=shape,
            advance=1400,
            instructions=(
                diagonal_font_hints.compile(shape, dict(shift=shift)).hex()
                if shift
                else ""
            ),
        )
        for char, name, shift in (
            ("A", "plain", 0),
            ("B", "contract", 16),
            ("C", "expand", -16),
        )
    ]
    items = font_probe.add_witnesses(items, "ABC")
    root.mkdir(parents=True, exist_ok=False)
    font = font_probe.font(items)
    (root / "probe.ttf").write_bytes(font)
    object_name = "R:ZP26R.TTF"
    configs = [
        (16, 16, "N"),
        (24, 24, "N"),
        (32, 32, "N"),
        (48, 48, "N"),
        (24, 32, "N"),
        (32, 24, "N"),
        (24, 32, "R"),
        (24, 32, "I"),
        (24, 32, "B"),
    ]
    pages = []
    for index, (w, h, rotation) in enumerate([(16, 16, "N")] + configs):
        selected = [
            g
            for g in items
            if (g["name"] in ("identity", "instruction-witness")) == (index == 0)
        ]
        probes = []
        for col, g in enumerate(selected):
            ax, ay = 24, 24 + h
            ax, ay = {
                "N": (ax, ay),
                "R": (128 - ay, ax),
                "I": (128 - ax, 128 - ay),
                "B": (ay, 128 - ax),
            }[rotation]
            probes.append(
                dict(
                    name=g["name"],
                    text=chr(g["codepoint"]),
                    origin="FT",
                    tile=[128 * col, 0, 128, 128],
                    anchor=[ax, ay],
                    width=w,
                    height=h,
                    orientation=rotation,
                    font=object_name,
                )
            )
        page = dict(
            name="00-state" if index == 0 else f"{index:02}-vector",
            group="state" if index == 0 else "geometry",
            canvas=[384, 128],
            probes=probes,
        )
        data = font_probe.zpl(page)
        (root / (page["name"] + ".zpl")).write_bytes(data)
        page["zpl_sha256"] = sha(data)
        pages.append(page)
    save(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object=object_name,
            font_sha256=sha(font),
            glyphs=items,
            pages=pages,
        ),
    )
    face = freetype.Face(str(root / "probe.ttf"))
    cases = []
    for x, y in sorted({(w, h) for w, h, _ in configs}):
        face.set_pixel_sizes(x, y)
        for char in "ABC":
            face.load_char(
                char,
                freetype.FT_LOAD_TARGET_MONO
                | freetype.FT_LOAD_NO_AUTOHINT
                | freetype.FT_LOAD_PEDANTIC,
            )
            cases.append(
                dict(
                    x=x,
                    y=y,
                    codepoint=ord(char),
                    advance=face.glyph.advance.x,
                    contours=[[[px, py, 1] for px, py in face.glyph.outline.points]],
                )
            )
    save(
        root / "freetype-outlines.json",
        dict(font_sha256=sha(font), freetype_version=freetype.version(), cases=cases),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    prepare(parser.parse_args().output)
