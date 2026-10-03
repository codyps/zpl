#!/usr/bin/env python3
"""Compare constructed fonts with saved native printer canvases, without printer I/O.

Research dependencies: freetype-py and Pillow; no core rendering dependency.
Uses only the calibration group in the development directory, never sealed data.
FreeType load flags and coordinate transforms:
https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html
https://freetype.org/freetype2/docs/reference/ft2-sizing_and_scaling.html#ft_set_transform
Capture geometry and font construction: zebra-http-api/examples/font_refine.
"""

import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path

import freetype
from PIL import Image


MATRICES = {
    # FreeType uses Y-up; these become clockwise rotations in printer Y-down.
    "N": (1, 0, 0, 1),
    "R": (0, 1, -1, 0),
    "I": (-1, 0, 0, -1),
    "B": (0, -1, 1, 0),
}
BASE = freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO
MODES = {
    "native": BASE | freetype.FT_LOAD_NO_AUTOHINT,
    "unhinted": BASE | freetype.FT_LOAD_NO_HINTING,
    "auto": BASE | freetype.FT_LOAD_FORCE_AUTOHINT,
    "default": BASE,
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_page(root, page):
    path = root / "development" / (page["name"] + ".png")
    require(
        sha256(path) == path.with_suffix(".sha256").read_text().strip(),
        f"PNG hash mismatch: {path}",
    )
    require(
        sha256(path.with_suffix(".zpl")) == page["zpl_sha256"],
        f"ZPL hash mismatch: {page['name']}",
    )
    with Image.open(path) as source:
        require(list(source.size) == page["canvas"], "Native dimensions changed")
        im = source.convert("RGB")
    points = set()
    for y in range(im.height):
        for x in range(im.width):
            pixel = im.getpixel((x, y))
            require(pixel in ((0, 0, 0), (255, 255, 255)), "Non-binary capture")
            if pixel == (0, 0, 0):
                points.add((x, y))
    return points


def render_glyph(font, probe, flags):
    require(
        probe["origin"] == "FT" and len(probe["text"]) == 1,
        "Expected one glyph at an explicit FT baseline",
    )
    # A fresh face isolates each case from previous size/glyph state. The report
    # does not claim to exercise interpreter lifecycle or cross-glyph state.
    face = freetype.Face(str(font))
    face.set_pixel_sizes(probe["width"], probe["height"])
    matrix = freetype.Matrix(*(v * 65536 for v in MATRICES[probe["orientation"]]))
    face.set_transform(matrix, freetype.Vector(0, 0))
    require(face.get_char_index(ord(probe["text"])) != 0, "Missing calibration glyph")
    face.load_char(probe["text"], flags)
    slot = face.glyph
    bitmap = slot.bitmap
    require(
        bitmap.pixel_mode == freetype.FT_PIXEL_MODE_MONO and bitmap.pitch >= 0,
        "Expected monochrome bitmap with nonnegative pitch",
    )
    buf = bytes(bitmap.buffer)
    tx, ty, tw, th = probe["tile"]
    ax, ay = probe["anchor"]
    points = {
        (tx + ax + slot.bitmap_left + x, ty + ay - slot.bitmap_top + y)
        for y in range(bitmap.rows)
        for x in range(bitmap.width)
        if buf[y * bitmap.pitch + x // 8] & (128 >> (x % 8))
    }
    require(
        all(tx <= x < tx + tw and ty <= y < ty + th for x, y in points),
        "Candidate glyph escapes its planned tile; no clipping permitted",
    )
    return points, [slot.advance.x, slot.advance.y]


def differences(reference, candidate):
    return dict(
        underpaint=len(reference - candidate),
        overpaint=len(candidate - reference),
        xor=len(reference ^ candidate),
        union=len(reference | candidate),
    )


def compare(root):
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    installed = json.loads((root / "calibration-install.json").read_text())
    fonts = {
        "R:ZRFN.TTF": root / "calibration-plain.ttf",
        "R:ZRFH.TTF": root / "calibration-shift.ttf",
    }
    require(
        {r["object"] for r in installed["fonts"]} == set(fonts),
        "Unexpected installed calibration fonts",
    )
    for record in installed["fonts"]:
        path = fonts[record["object"]]
        require(
            path.stat().st_size == record["bytes"] and sha256(path) == record["sha256"],
            f"Installed font provenance differs: {path}",
        )
    pages = [p for p in manifest["pages"] if p["group"] == "calibration"]
    require(len(pages) == 20, "Expected the recorded 20-page calibration campaign")
    captures = {p["name"]: load_page(root, p) for p in pages}

    # Independent observed control: SHPIX translates all printer ink one device
    # dot along the glyph's X axis, including its rotated direction. No alignment
    # is inferred or optimized. This also checks the manifest's rotation mapping.
    shift_controls = []
    for page in pages:
        p = page["probes"][0]
        if p["font"] != "R:ZRFN.TTF":
            continue
        shifted = next(
            q
            for q in pages
            if q["probes"][0]["font"] == "R:ZRFH.TTF"
            and all(
                q["probes"][0][k] == p[k] for k in ("height", "width", "orientation")
            )
        )
        require(page["canvas"] == shifted["canvas"], "Shift control canvas differs")
        require(
            [
                {k: v for k, v in probe.items() if k != "font"}
                for probe in page["probes"]
            ]
            == [
                {k: v for k, v in probe.items() if k != "font"}
                for probe in shifted["probes"]
            ],
            "Shift control placement differs",
        )
        dx, dy = {"N": (1, 0), "R": (0, 1), "I": (-1, 0), "B": (0, -1)}[
            p["orientation"]
        ]
        expected = {(x + dx, y + dy) for x, y in captures[page["name"]]}
        xor = len(expected ^ captures[shifted["name"]])
        require(xor == 0, "Recorded printer SHPIX control failed")
        shift_controls.append(
            dict(plain=page["name"], shifted=shifted["name"], xor=xor)
        )

    results = []
    for page in pages:
        reference = captures[page["name"]]
        for mode, flags in MODES.items():
            candidate = set()
            covered_reference = set()
            glyphs = []
            for p in page["probes"]:
                points, advance = render_glyph(fonts[p["font"]], p, flags)
                tx, ty, tw, th = p["tile"]
                ref = {
                    (x, y)
                    for x, y in reference
                    if tx <= x < tx + tw and ty <= y < ty + th
                }
                require(
                    not candidate & points and not covered_reference & ref,
                    "Calibration glyph tiles overlap",
                )
                candidate |= points
                covered_reference |= ref
                glyphs.append(
                    dict(
                        glyph=p["text"],
                        **differences(ref, points),
                        advance_26_6=advance,
                    )
                )
            require(
                covered_reference == reference, "Unassigned ink outside glyph tiles"
            )
            p = page["probes"][0]
            results.append(
                dict(
                    page=page["name"],
                    font=p["font"],
                    height=p["height"],
                    width=p["width"],
                    orientation=p["orientation"],
                    mode=mode,
                    **differences(reference, candidate),
                    exact_glyphs=sum(g["xor"] == 0 for g in glyphs),
                    glyphs=glyphs,
                )
            )
    return dict(
        schema="constructed-font-freetype-v1",
        freetype_version=freetype.version(),
        python_packages={name: version(name) for name in ("freetype-py", "Pillow")},
        manifest_sha256=sha256(manifest_path),
        font_sources=installed["fonts"],
        load_flags=MODES,
        face_lifecycle="new face for each glyph",
        coordinate_policy="original full canvas; no alignment, clipping, or resampling",
        sealed_pixels_read=False,
        printer_shift_controls=shift_controls,
        comparisons=results,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_root", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="New report file")
    args = parser.parse_args()
    require(not args.output.exists(), "Report exists; choose a new output file")
    report = compare(args.capture_root)
    with args.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    for font in ("R:ZRFN.TTF", "R:ZRFH.TTF"):
        for mode in MODES:
            rows = [
                r
                for r in report["comparisons"]
                if r["font"] == font and r["mode"] == mode
            ]
            xor = sum(r["xor"] for r in rows)
            union = sum(r["union"] for r in rows)
            count = sum(len(r["glyphs"]) for r in rows)
            exact = sum(r["exact_glyphs"] for r in rows)
            print(
                f"{font} {mode}: XOR={xor}, exact ink={exact}/{count}, IoU={1 - xor / union:.8f}"
            )


if __name__ == "__main__":
    main()
