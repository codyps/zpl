"""Original font exposing ROUND thresholds and fractional scaled CVT values.

OpenType instruction set: ROUND, RCVT, FLOOR, SUB, MUL, SCFS.
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import argparse
from pathlib import Path
import struct

import font_probe as build
from capture_font_probe import save, save_json, sha


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    items = build.glyphs()[:2]

    def add(name, expression, family):
        items.append(
            dict(
                name=name,
                codepoint=32 + len(items),
                contours=[build.rect(0, 0, 1024, 256)],
                instructions=build.set_right(expression).hex(),
                advance=1024,
                family=family,
            )
        )

    add("instruction-witness", build.push(13 * 64), "state")
    for color in range(3):
        for phase in range(24, 41):
            add(
                f"round-{color}-{phase}",
                build.push(128 + phase)
                + bytes([0x68 + color])
                + build.push(512)
                + bytes([0x63]),
                "round",
            )
    values = [1473, 993, 1757, 735, 737]
    for i, v in enumerate(values):
        # RCVT duplicate floor subtract extracts fractional dot, magnified 64x.
        add(
            f"cvt-fraction-{v}",
            build.push(i)
            + bytes([0x45, 0x20, 0x66, 0x61])
            + build.push(4096)
            + bytes([0x63]),
            "cvt",
        )
    font = build.font(items, {b"cvt ": bytearray(struct.pack(">5h", *values))})
    save(root / "probe.ttf", font)
    pages = []

    def page(name, selected, size, side):
        cols = 384 // side
        height = ((len(selected) + cols - 1) // cols) * side
        probes = []
        zpl = f"^XA^PW384^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for i, g in enumerate(selected):
            tx, ty = (i % cols) * side, (i // cols) * side
            ax, ay = 8, 16 + size
            # The installation verifier expects the original state's 16-dot anchor.
            if name == "00-state":
                ax = 16
            probes.append(
                dict(
                    name=g["name"],
                    text=chr(g["codepoint"]),
                    tile=[tx, ty, side, side],
                    anchor=[ax, ay],
                    height=size,
                    width=size,
                    orientation="N",
                    origin="FT",
                )
            )
            field = "".join(f"_{b:02X}" for b in chr(g["codepoint"]).encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@N,{size},{size},R:ZP26B.TTF^FH_^FD{field}^FS"
        data = (zpl + "^XZ").encode()
        save(root / (name + ".zpl"), data)
        pages.append(
            dict(
                name=name,
                group="rounding",
                canvas=[384, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )

    page("00-state", items[1:3], 16, 64)
    page("round-16", [g for g in items if g["family"] == "round"], 16, 64)
    page("round-32", [g for g in items if g["family"] == "round"], 32, 64)
    page("cvt-16", [g for g in items if g["family"] == "cvt"], 16, 96)
    page("cvt-32", [g for g in items if g["family"] == "cvt"], 32, 96)
    save_json(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object="R:ZP26B.TTF",
            font_sha256=sha(font),
            glyphs=items,
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    prepare(parser.parse_args().root)
