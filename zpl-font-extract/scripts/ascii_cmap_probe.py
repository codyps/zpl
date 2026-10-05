"""Verify printable ASCII and private-use hint witnesses on a development printer.

Each visible codepoint has a distinct seven-bar design. Text sequences exercise
space and advances; this is a cmap/layout control, not Font 0 accuracy evidence.
"""

import argparse
from pathlib import Path

from ascii_font_probe import PRINTABLE, PREFIX
from capture_font_probe import save, save_json, sha
import font_probe


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    items = [
        dict(
            name=f"code-{ord(c)}",
            codepoint=ord(c),
            instructions="",
            advance=512 + 8 * ord(c),
            contours=(
                []
                if c == " "
                else [
                    font_probe.rect(i * 256, 0, 128, 1024 if ord(c) & (1 << i) else 256)
                    for i in range(7)
                ]
            ),
        )
        for c in PRINTABLE
    ]
    items = font_probe.add_witnesses(items, PRINTABLE)
    data = font_probe.font(items)
    save(root / "probe.ttf", data)
    witnesses = font_probe.witness_codepoints(PRINTABLE)
    pages = [
        dict(
            name="00-state",
            canvas=[384, 64],
            group="state",
            probes=[
                dict(
                    name=name,
                    text=chr(witnesses[i]),
                    tile=[64 * i, 0, 64, 64],
                    anchor=[16, 32],
                    width=16,
                    height=16,
                    orientation="N",
                    origin="FT",
                )
                for i, name in enumerate(("identity", "instruction-witness"))
            ],
        ),
        dict(
            name="01-ascii",
            canvas=[384, 256],
            group="mapping",
            probes=[
                dict(
                    name=f"code-{ord(c)}",
                    text=c,
                    tile=[i % 12 * 32, i // 12 * 32, 32, 32],
                    anchor=[8, 20],
                    width=16,
                    height=16,
                    orientation="N",
                    origin="FT",
                )
                for i, c in enumerate(PRINTABLE)
            ],
        ),
        dict(
            name="02-spacing",
            canvas=[384, 384],
            group="layout",
            probes=[
                dict(
                    name=f"text-{i}",
                    text=text,
                    tile=[0, 64 * i, 384, 64],
                    anchor=[16, 40],
                    width=16,
                    height=16,
                    orientation="N",
                    origin="FT",
                )
                for i, text in enumerate(
                    ("A A", "A  A", "| |", '!"#~_^', "0123456789", "abcdefghijklmnop")
                )
            ],
        ),
    ]
    for page in pages:
        width, height = page["canvas"]
        zpl = f"^XA^PW{width}^LL{height}" + PREFIX
        for p in page["probes"]:
            tx, ty, _, _ = p["tile"]
            ax, ay = p["anchor"]
            encoded = "".join(f"_{b:02X}" for b in p["text"].encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@N,16,16,R:ZP26M.TTF^FH_^FD{encoded}^FS"
        request = (zpl + "^XZ").encode()
        save(root / (page["name"] + ".zpl"), request)
        page["zpl_sha256"] = sha(request)
    save_json(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object="R:ZP26M.TTF",
            font_sha256=sha(data),
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    prepare(parser.parse_args().root)
