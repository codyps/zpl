"""Plan native Font 0 targets for reconstruction; capture is a separate action.

Zebra ^A0/^FT/^FH command reference:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
Use font0_outline_probe.capture(..., schema='small-font-probe-v1') for the
existing serialized, identity-checked preview transport with repeated controls.
"""

import argparse
import math
from pathlib import Path

from capture_font_probe import save, save_json, sha
from font0_outline_probe import capture


def prepare(root, characters, configurations, group):
    if (
        not characters
        or len(set(characters)) != len(characters)
        or any(not 33 <= ord(c) <= 126 for c in characters)
    ):
        raise ValueError("supply unique visible ASCII glyphs")
    if (
        not configurations
        or len(set(configurations)) != len(configurations)
        or len(configurations) > 32
    ):
        raise ValueError("supply 1..32 distinct size/rotation configurations")
    if group not in ("development", "validation"):
        raise ValueError("unknown dataset group")
    if any(
        not (1 <= w <= 512 and 1 <= h <= 512 and r in "NRIB" and len(r) == 1)
        for w, h, r in configurations
    ):
        raise ValueError("invalid dimensions or rotation")
    size = max(max(w, h) for w, h, _ in configurations)
    # Large descenders need more than a fixed 64-dot bottom margin. Allocate
    # their space in the native request, not by padding or aligning a capture.
    padding = 16 if size <= 64 else math.ceil(size / 5) + 16
    tile = math.ceil((size + 2 * padding) / 32) * 32
    width = 384 if tile <= 128 else 768
    columns = width // tile
    per_page = columns * (1536 // tile)
    fields = [(w, h, r, c) for w, h, r in configurations for c in characters]
    if math.ceil(len(fields) / per_page) + 3 > 32:
        raise ValueError("campaign exceeds 32 previews including repeated controls")
    root.mkdir(parents=True, exist_ok=False)
    pages = []
    for offset in range(0, len(fields), per_page):
        chunk = fields[offset : offset + per_page]
        height = math.ceil(len(chunk) / columns) * tile
        probes = []
        zpl = f"^XA^PW{width}^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for i, (w, h, rotation, char) in enumerate(chunk):
            tx, ty = i % columns * tile, i // columns * tile
            ax, ay = padding, padding + h
            ax, ay = {
                "N": (ax, ay),
                "R": (tile - ay, ax),
                "I": (tile - ax, tile - ay),
                "B": (ay, tile - ax),
            }[rotation]
            probes.append(
                dict(
                    text=char,
                    width=w,
                    height=h,
                    orientation=rotation,
                    origin="FT",
                    tile=[tx, ty, tile, tile],
                    anchor=[ax, ay],
                )
            )
            zpl += f"^FT{tx+ax},{ty+ay}^A0{rotation},{h},{w}^FH_^FD_{ord(char):02X}^FS"
        data = (zpl + "^XZ").encode()
        name = f"reconstruction-{len(pages):02}"
        save(root / (name + ".zpl"), data)
        pages.append(
            dict(
                name=name,
                font="0",
                group=group,
                canvas=[width, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )
    save_json(
        root / "manifest.json",
        dict(
            schema="small-font-probe-v1",
            characters=characters,
            configurations=configurations,
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "capture"))
    parser.add_argument("root", type=Path)
    parser.add_argument("--characters")
    parser.add_argument("--sizes", help="comma-separated square sizes")
    parser.add_argument(
        "--transforms", default="", help="comma-separated WIDTHxHEIGHTxN/R/I/B"
    )
    parser.add_argument(
        "--group", choices=("development", "validation"), default="validation"
    )
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    args = parser.parse_args()
    if args.action == "prepare":
        if not args.characters:
            parser.error("prepare requires --characters")
        configurations = [
            (int(s), int(s), "N") for s in (args.sizes or "").split(",") if s
        ]
        for transform in args.transforms.split(","):
            if transform:
                w, h, rotation = transform.split("x")
                configurations.append((int(w), int(h), rotation))
        prepare(args.root, args.characters, configurations, args.group)
    else:
        capture(args.root, args.host, schema="small-font-probe-v1")
