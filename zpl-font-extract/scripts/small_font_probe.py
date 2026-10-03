"""Small-size Font 0 reconstruction targets and known-Swiss controls.

ZPL ^A0/^A@/^FT: Zebra Programming Guide command reference.
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Development and reserved sizes are selected before observing the previews.
"""

import argparse
from pathlib import Path

from capture_font_probe import save, save_json, sha
from font0_outline_probe import capture
from swiss_probe import FONT, FONT_SHA


def prepare(root, final_validation=False):
    root.mkdir(parents=True, exist_ok=False)
    pages = []

    def page(name, font, rows, group):
        probes = []
        height = ((sum(len(text) for _, _, _, text in rows) + 5) // 6) * 64
        zpl = f"^XA^PW384^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for w, h, orientation, text in rows:
            for char in text:
                i = len(probes)
                tx, ty = i % 6 * 64, i // 6 * 64
                ax, ay = 16, 16 + h
                ax, ay = {
                    "N": (ax, ay),
                    "R": (64 - ay, ax),
                    "I": (64 - ax, 64 - ay),
                    "B": (ay, 64 - ax),
                }[orientation]
                probes.append(
                    dict(
                        text=char,
                        width=w,
                        height=h,
                        orientation=orientation,
                        origin="FT",
                        tile=[tx, ty, 64, 64],
                        anchor=[ax, ay],
                    )
                )
                selector = (
                    f"^A0{orientation},{h},{w}"
                    if font == "0"
                    else f"^A@{orientation},{h},{w},{FONT}"
                )
                encoded = "".join(f"_{b:02X}" for b in char.encode())
                zpl += f"^FT{tx+ax},{ty+ay}{selector}^FH_^FD{encoded}^FS"
        data = (zpl + "^XZ").encode()
        save(root / (name + ".zpl"), data)
        pages.append(
            dict(
                name=name,
                font=font,
                group=group,
                canvas=[384, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )

    chars = "HOSgj@"
    if final_validation:
        page(
            "font0-unseen-sizes",
            "0",
            [(s, s, "N", chars) for s in [22, 25, 26, 29, 30, 34, 36, 38, 40]],
            "validation",
        )
        page(
            "font0-unseen-transforms",
            "0",
            [
                (w, h, r, chars)
                for w, h, r in [
                    (15, 25, "N"),
                    (25, 15, "N"),
                    (22, 30, "N"),
                    (30, 22, "N"),
                    (19, 19, "R"),
                    (25, 25, "B"),
                ]
            ],
            "validation",
        )
        save_json(
            root / "manifest.json", dict(schema="small-font-probe-v1", pages=pages)
        )
        return
    page(
        "font0-small-development",
        "0",
        [(s, s, "N", chars) for s in [10, 12, 14, 16, 18, 20, 24, 28, 32]],
        "development",
    )
    page(
        "font0-small-validation",
        "0",
        [(s, s, "N", chars) for s in [11, 13, 15, 17, 19, 21, 23, 27, 31]],
        "validation",
    )
    page(
        "font0-transform-development",
        "0",
        [
            (w, h, r, chars)
            for w, h, r in [
                (12, 24, "N"),
                (24, 12, "N"),
                (16, 24, "N"),
                (24, 16, "N"),
                (14, 14, "R"),
                (20, 20, "I"),
            ]
        ],
        "development",
    )
    page(
        "font0-transform-validation",
        "0",
        [
            (w, h, r, chars)
            for w, h, r in [
                (13, 23, "N"),
                (23, 13, "N"),
                (17, 21, "N"),
                (21, 17, "N"),
                (17, 17, "B"),
                (23, 23, "R"),
            ]
        ],
        "validation",
    )
    for group, sizes in [
        ("development", [10, 12, 14, 18, 24]),
        ("validation", [11, 13, 17, 21]),
    ]:
        for size in sizes:
            page(
                f"swiss-{size}-{group}",
                "swiss",
                [(size, size, "N", "".join(map(chr, range(32, 127))))],
                group,
            )
    save_json(
        root / "manifest.json",
        dict(
            schema="small-font-probe-v1",
            swiss_object=FONT,
            swiss_sha256=FONT_SHA,
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "capture"])
    parser.add_argument("root", type=Path)
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    parser.add_argument("--final-validation", action="store_true")
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(args.root, args.final_validation)
    else:
        capture(args.root, args.host, schema="small-font-probe-v1")
