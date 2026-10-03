"""Expose fractional CVT scaling under independent X/Y sizes.

RCVT/SCFS/FLOOR semantics follow the OpenType TrueType instruction reference:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import argparse
from pathlib import Path
import struct

import font_probe as build
from capture_font_probe import save, save_json, sha


def prepare(root, validation=False, object_name="R:ZP26D.TTF"):
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
    values = [
        127,
        128,
        129,
        255,
        256,
        257,
        511,
        512,
        513,
        735,
        736,
        737,
        1023,
        1024,
        1025,
        1473,
        1535,
        1536,
        1537,
        1757,
        2047,
        2048,
        2049,
    ]
    if validation:
        values = [
            91,
            127,
            272,
            453,
            635,
            816,
            896,
            1077,
            1360,
            1536,
            1685,
            1730,
            1775,
            1821,
            1866,
            1911,
            1944,
            1957,
            1989,
            2002,
            2034,
            2048,
            2080,
        ]
    for index, value in enumerate(values):
        for axis in ("x", "y"):
            for part in ("integer", "fraction"):
                expr = (
                    bytes([0x01 if axis == "x" else 0x00])
                    + build.push(index)
                    + bytes([0x45])
                )
                expr += (
                    bytes([0x66])
                    if part == "integer"
                    else bytes([0x20, 0x66, 0x61]) + build.push(4096) + bytes([0x63])
                )
                code = build.set_right(expr)
                for point in [1, 2]:
                    code += (
                        bytes([0x00])
                        + build.push(point)
                        + build.push(128)
                        + bytes([0x48])
                    )
                items.append(
                    dict(
                        name=f"cvt-{value}-{axis}-{part}",
                        codepoint=32 + len(items),
                        contours=[build.rect(0, 0, 1024, 256)],
                        instructions=code.hex(),
                        advance=2048,
                        family="cvt",
                    )
                )
    data = build.font(
        items, {b"cvt ": bytearray(struct.pack(">" + str(len(values)) + "h", *values))}
    )
    save(root / "probe.ttf", data)
    pages = []

    def page(name, probes, height):
        zpl = f"^XA^PW384^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for p in probes:
            tx, ty, _, _ = p["tile"]
            ax, ay = p["anchor"]
            encoded = "".join(f"_{b:02X}" for b in p["text"].encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@N,{p['height']},{p['width']},{object_name}^FH_^FD{encoded}^FS"
        zpl = (zpl + "^XZ").encode()
        save(root / (name + ".zpl"), zpl)
        pages.append(
            dict(
                name=name,
                group="cvt-axis",
                canvas=[384, height],
                probes=probes,
                zpl_sha256=sha(zpl),
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
    for w, h in (
        [(13, 23), (23, 13), (17, 29), (29, 17)]
        if validation
        else [(19, 32), (32, 19), (12, 24), (24, 12)]
    ):
        probes = [
            dict(
                name=g["name"],
                text=chr(g["codepoint"]),
                tile=[i % 3 * 128, i // 3 * 8, 128, 8],
                anchor=[8, 4],
                width=w,
                height=h,
                orientation="N",
                origin="FT",
            )
            for i, g in enumerate(items[3:])
        ]
        page(f"cvt-{w}-{h}", probes, ((len(probes) + 2) // 3) * 8)
    save_json(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object=object_name,
            font_sha256=sha(data),
            glyphs=items,
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--validation", action="store_true")
    parser.add_argument(
        "--object", choices=["R:ZP26D.TTF", "R:ZP26E.TTF"], default="R:ZP26D.TTF"
    )
    args = parser.parse_args()
    prepare(args.root, args.validation, args.object)
