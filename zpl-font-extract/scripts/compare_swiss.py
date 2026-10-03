"""Compare Swiss previews on native canvases; validation pages are opt-in.

No alignment, cropping of canvases, rescaling, or external-font bundling.
FreeType is a diagnostic reference, never a runtime dependency.
"""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import subprocess

from analyze_font_probe import ink, calibration
from capture_font_probe import sha
from swiss_probe import FONT_SHA


def counts(reference, candidate):
    return dict(
        under=len(reference - candidate),
        over=len(candidate - reference),
        xor=len(reference ^ candidate),
        union=len(reference | candidate),
    )


def advance(row, policy):
    if policy == "device":
        return row["layout_advance"]
    if policy == "native":
        return (row["advance"] + 32) // 64
    linear = row["linear_advance"]
    if policy == "linear":
        return (linear + 32) // 64
    if policy == "ceil":
        return (linear + 63) // 64
    if policy == "sixteenth":
        return ((linear + 2) // 4 + 8) // 16
    raise ValueError(policy)


def pixels(row):
    return {
        (row["left"] + x, row["top"] + y)
        for y, hexrow in enumerate(row["rows"])
        for x in range(row["width"])
        if bytes.fromhex(hexrow)[x // 8] & (128 >> (x % 8))
    }


def compare(root, font, engine, validation=False):
    assert sha(font.read_bytes()) == FONT_SHA
    manifest = json.loads((root / "manifest.json").read_text())
    capture = json.loads((root / "zd621/capture.json").read_text())
    assert (
        capture["status"] == "complete"
        and capture["swiss_repeat_exact"]
        and capture["resident_repeat_exact"]
    )
    assert capture["manifest_sha256"] == sha((root / "manifest.json").read_bytes())
    pages = [
        p
        for p in manifest["pages"]
        if p["group"] == ("validation" if validation else "development")
    ]
    queries = sorted(
        {
            (p["width"], p["height"], ord(c), "NRIB".index(p["orientation"]))
            for page in pages
            for p in page["probes"]
            for c in p["text"]
        }
    )
    engines = {}
    for mode in ["center", "zebra", "zd621"]:
        result = subprocess.run(
            [str(engine.resolve()), "glyphs", str(font), mode],
            input="".join(" ".join(map(str, q)) + "\n" for q in queries),
            text=True,
            capture_output=True,
            check=True,
            timeout=120,
        )
        rows = [json.loads(s) for s in result.stdout.splitlines()]
        engines[mode] = dict(zip(queries, rows, strict=True))
    report = dict(
        schema="swiss-native-comparison-v1",
        font_sha256=FONT_SHA,
        validation=validation,
        printer=capture["printer"],
        manifest_sha256=capture["manifest_sha256"],
        pages=[],
        totals={},
    )
    totals = defaultdict(lambda: defaultdict(int))
    for page in pages:
        name = page["name"]
        record = next(p for p in capture["pages"] if p["name"] == name)
        image = root / "zd621" / (name + ".png")
        assert sha(image.read_bytes()) == record["png_sha256"]
        assert (
            sha((root / "zd621" / (name + ".zpl")).read_bytes())
            == page["zpl_sha256"]
            == record["zpl_sha256"]
        )
        reference = ink(image, page["canvas"])
        candidates = defaultdict(set)
        cases = defaultdict(list)
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            expected = {
                (x, y) for x, y in reference if tx <= x < tx + tw and ty <= y < ty + th
            }
            for mode, rows in engines.items():
                for policy in ["native", "linear", "ceil", "sixteenth", "device"]:
                    key = mode + "-" + policy
                    candidate = set()
                    pen = 0
                    turn = "NRIB".index(p["orientation"])
                    for ch in p["text"]:
                        row = rows[(p["width"], p["height"], ord(ch), turn)]
                        if "error" in row:
                            raise ValueError((name, ch, row))
                        dx, dy = [(pen, 0), (0, pen), (-pen, 0), (0, -pen)][turn]
                        candidate.update(
                            (tx + ax + x + dx, ty + ay + y + dy) for x, y in pixels(row)
                        )
                        pen += advance(row, policy)
                    assert all(
                        tx <= x < tx + tw and ty <= y < ty + th for x, y in candidate
                    ), (name, p["text"], key, "candidate escapes tile")
                    candidates[key].update(candidate)
                    stats = counts(expected, candidate)
                    cases[key].append(dict(text=p["text"], **stats))
            # Independent reference only for isolated glyphs; string metrics
            # are tested with explicit policies above, not assumed by FreeType.
            if len(p["text"]) == 1:
                candidate, _ = calibration.render_glyph(
                    font, p, calibration.MODES["native"]
                )
                candidates["freetype"].update(candidate)
                cases["freetype"].append(
                    dict(text=p["text"], **counts(expected, candidate))
                )
        results = {}
        for key, candidate in candidates.items():
            stats = counts(reference, candidate)
            stats.update(
                exact=sum(c["xor"] == 0 for c in cases[key]), cases=len(cases[key])
            )
            stats["iou"] = 1 - stats["xor"] / stats["union"] if stats["union"] else 1
            results[key] = dict(**stats, glyphs=cases[key])
            category = "spacing" if name.startswith("spacing") else "glyphs"
            for field in ["under", "over", "xor", "union", "exact", "cases"]:
                totals[category + "/" + key][field] += stats[field]
        report["pages"].append(dict(name=name, results=results))
    for key, stats in totals.items():
        stats["iou"] = 1 - stats["xor"] / stats["union"] if stats["union"] else 1
        report["totals"][key] = dict(stats)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("font", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--validation", action="store_true")
    args = parser.parse_args()
    result = compare(args.root, args.font, args.engine, args.validation)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result["totals"], indent=2))
