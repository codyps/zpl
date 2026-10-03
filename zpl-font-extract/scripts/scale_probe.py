"""Expose effective CVT/outline scaling and advance rounding with original hints.

GC[1] reads original coordinates; RCVT reads a design-unit CVT scaled to 26.6.
Integer and fractional parts are separate integer-width bars. Every bar is forced
to two dots high. OpenType GC, RCVT, FLOOR, SCFS and arithmetic instructions:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import argparse
from pathlib import Path
import struct

import font_probe as build
from capture_font_probe import save, save_json, sha


def prepare(root, start_size=1):
    root.mkdir(parents=True, exist_ok=False)
    items = build.glyphs()[:2]
    items.append(
        dict(
            name="instruction-witness",
            codepoint=34,
            contours=[build.rect(0, 0, 1024, 256)],
            instructions=build.set_right(build.push(13 * 64)).hex(),
            advance=1024,
            family="state",
        )
    )
    for source in ["cvt", "outline"]:
        for axis in ["x", "y"]:
            for part in ["integer", "fraction"]:
                expr = (
                    bytes([0x01 if axis == "x" else 0x00])
                    + build.push(0 if source == "cvt" else 2)
                    + bytes([0x45 if source == "cvt" else 0x47])
                )
                expr += (
                    bytes([0x66])
                    if part == "integer"
                    else bytes([0x20, 0x66, 0x61]) + build.push(4096) + bytes([0x63])
                )
                program = build.set_right(expr)
                for p in [1, 2]:
                    program += (
                        bytes([0x00]) + build.push(p) + build.push(128) + bytes([0x48])
                    )
                items.append(
                    dict(
                        name=f"{source}-{axis}-{part}",
                        codepoint=32 + len(items),
                        contours=[build.rect(0, 0, 2048, 2048)],
                        instructions=program.hex(),
                        advance=2048,
                        family="scale",
                    )
                )
    for adv in range(700, 771, 2):
        items.append(
            dict(
                name=f"advance-{adv}",
                codepoint=32 + len(items),
                contours=[build.rect(0, 0, 128, 512)],
                instructions="",
                advance=adv,
                family="advance",
            )
        )
    items.append(
        dict(
            name="sentinel",
            codepoint=32 + len(items),
            contours=[build.rect(0, 0, 128, 1024)],
            instructions="",
            advance=384,
            family="sentinel",
        )
    )
    font = build.font(items, {b"cvt ": bytearray(struct.pack(">h", 2048))})
    save(root / "probe.ttf", font)
    pages = []

    def page(name, probes, height):
        zpl = f"^XA^PW384^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for p in probes:
            tx, ty, _, _ = p["tile"]
            ax, ay = p["anchor"]
            text = "".join(f"_{b:02X}" for b in p["text"].encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@N,{p['height']},{p['width']},R:ZP26C.TTF^FH_^FD{text}^FS"
        data = (zpl + "^XZ").encode()
        save(root / (name + ".zpl"), data)
        pages.append(
            dict(
                name=name,
                group="scaling",
                canvas=[384, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )

    page(
        "00-state",
        [
            dict(
                name=g["name"],
                text=chr(g["codepoint"]),
                tile=[i * 64, 0, 64, 64],
                anchor=[16, 32],
                width=16,
                height=16,
                orientation="N",
                origin="FT",
            )
            for i, g in enumerate(items[1:3])
        ],
        64,
    )
    for source in ["cvt", "outline"]:
        for axis in ["x", "y"]:
            probes = []
            for row, size in enumerate(range(start_size, start_size + 64)):
                for i, part in enumerate(["integer", "fraction"]):
                    g = next(g for g in items if g["name"] == f"{source}-{axis}-{part}")
                    probes.append(
                        dict(
                            name=g["name"],
                            text=chr(g["codepoint"]),
                            tile=[i * 192, row * 8, 192, 8],
                            anchor=[8, 4],
                            width=size,
                            height=size,
                            orientation="N",
                            origin="FT",
                        )
                    )
            page(f"{source}-{axis}", probes, 512)
    for x, y in [(16, 16), (32, 32), (19, 32)] if start_size == 1 else []:
        selected = [g for g in items if g["family"] == "advance"]
        sentinel = chr(items[-1]["codepoint"])
        probes = [
            dict(
                name=g["name"],
                text=chr(g["codepoint"]) * 4 + sentinel,
                tile=[i % 4 * 96, i // 4 * 64, 96, 64],
                anchor=[8, 16 + y],
                width=x,
                height=y,
                orientation="N",
                origin="FT",
            )
            for i, g in enumerate(selected)
        ]
        page(f"advances-{x}-{y}", probes, ((len(selected) + 3) // 4) * 64)
    save_json(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object="R:ZP26C.TTF",
            font_sha256=sha(font),
            glyphs=items,
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--start-size", type=int, choices=[1, 65], default=1)
    args = parser.parse_args()
    prepare(args.root, args.start_size)
