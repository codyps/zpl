#!/usr/bin/env python3
"""Export printer-selected QR masks and all eight local candidate matrices.

Build first: direnv exec . cargo build -p zpl-cmd --locked
Requires Pillow. This is offline diagnosis, not the full-canvas accuracy gate.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

from PIL import Image, ImageChops


ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "zpl/tests/fixtures/qr-zd621-v1"
COMMAND = re.compile(r"\^BQN,([12]),(\d+),([LMQH]),([0-7])(?=\^)")


def matrix(image, index, fields, scale):
    # Regions identify atlas symbols for analysis only. The Rust regression
    # tests compare the entire unmodified printer canvas without registration.
    if fields == 1:
        region = image
    else:
        x, y = index % 8 * 104, index // 8 * 100
        region = image.crop((x, y, x + 104, y + 100))
    ink = region.point(lambda v: 255 if v < 128 else 0)
    bounds = ink.getbbox()
    if bounds is None:
        raise ValueError("QR control contains no ink")
    x, y, right, bottom = bounds
    if right - x != bottom - y or (right - x) % scale:
        raise ValueError("QR control does not have a square module grid")
    side = (right - x) // scale
    if side < 21 or (side - 17) % 4:
        raise ValueError("invalid QR module count")
    return [
        [int(region.getpixel((x + col * scale, y + row * scale)) < 128)
         for col in range(side)]
        for row in range(side)
    ]


def captured_mask(modules, model):
    # ISO/IEC 18004:2000 §8.9, Annex C.1 and Annex M.9: format placement,
    # BCH generator, and the distinct Model 1 / Model 2 format XOR masks.
    # https://www.iso.org/standard/30789.html
    coordinates = [(8, i) for i in range(6)] + [(8, 7), (8, 8), (7, 8)]
    coordinates += [(14 - i, 8) for i in range(9, 15)]
    bits = sum(modules[y][x] << bit for bit, (x, y) in enumerate(coordinates))
    bits ^= 0x2825 if model == 1 else 0x5412
    remainder = bits
    for bit in range(14, 9, -1):
        if remainder & (1 << bit):
            remainder ^= 0x537 << (bit - 10)
    if remainder:
        raise ValueError("captured QR format bits fail BCH validation")
    return (bits >> 10) & 7


def export(renderer):
    records = []
    with tempfile.TemporaryDirectory(prefix="zpl-qr-masks-") as temporary:
        request, response = Path(temporary) / "input.zpl", Path(temporary) / "local.png"
        with (FIXTURES / "manifest.tsv").open() as manifest:
            rows = list(csv.DictReader(manifest, delimiter="\t"))
        for row in rows:
            name = row["name"]
            source = (FIXTURES / f"{name}.zpl").read_bytes()
            png = (FIXTURES / f"{name}.png").read_bytes()
            for data, key in [(source, "input_sha256"), (png, "printer_sha256")]:
                if hashlib.sha256(data).hexdigest() != row[key]:
                    raise ValueError(f"{name}: changed {key}")
            source = source.decode("ascii")
            commands = list(COMMAND.finditer(source))
            if name.startswith("placement-"):
                fields = 24
            elif name.startswith("byte-"):
                fields = 12
            else:
                fields = 1
            if len(commands) != fields:
                raise ValueError(f"{name}: unexpected QR command count")
            with Image.open(FIXTURES / f"{name}.png") as image:
                reference = image.convert("L")
            batch = []
            for index, command in enumerate(commands):
                modules = matrix(reference, index, fields, int(command[2]))
                batch.append({
                    "frame": name, "field": index, "model": int(command[1]),
                    "level": command[3], "mask": captured_mask(modules, int(command[1])),
                    "input_sha256": row["input_sha256"],
                    "printer_sha256": row["printer_sha256"],
                    "printer_modules": modules, "candidates": [],
                })
            for mask in range(8):
                changed = COMMAND.sub(lambda m: m[0][:-1] + str(mask), source)
                request.write_text(changed)
                subprocess.run([str(renderer), "render", str(request), str(response), "--explicit-qr-mask"], check=True,
                               capture_output=True, timeout=30)
                with Image.open(response) as image:
                    local = image.convert("L")
                if local.size != reference.size:
                    raise ValueError(f"{name}: canvas size differs")
                for index, command in enumerate(commands):
                    candidate = matrix(local, index, fields, int(command[2]))
                    if captured_mask(candidate, int(command[1])) != mask:
                        raise ValueError(f"{name}: renderer did not honor requested mask {mask}")
                    batch[index]["candidates"].append(candidate)
            for record in batch:
                if record["candidates"][record["mask"]] != record["printer_modules"]:
                    raise ValueError(f"{name}: encoding differs in field {record['field']}")
            # Also verify all fields together at their original positions.
            selected = iter(record["mask"] for record in batch)
            request.write_text(COMMAND.sub(lambda m: m[0][:-1] + str(next(selected)), source))
            subprocess.run([str(renderer), "render", str(request), str(response), "--explicit-qr-mask"], check=True,
                           capture_output=True, timeout=30)
            with Image.open(response) as image:
                local = image.convert("L")
            binary = lambda im: im.point(lambda v: 0 if v < 128 else 255)
            if local.size != reference.size or ImageChops.difference(binary(local), binary(reference)).getbbox():
                raise ValueError(f"{name}: captured-mask full-canvas rendering differs")
            records.extend(batch)
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--renderer", type=Path, default=ROOT / "target/debug/zpl-cmd")
    args = parser.parse_args()
    records = export(args.renderer.resolve())
    args.output.write_text(json.dumps({"schema": 1, "symbols": records}) + "\n")
    print(f"Verified and exported {len(records)} QR symbols to {args.output}")


if __name__ == "__main__":
    main()
