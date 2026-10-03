"""Full native-canvas comparison for original rounding/scaling probe fonts."""

import argparse
import json
from pathlib import Path
import subprocess

from analyze_font_probe import ink
from capture_font_probe import sha
from compare_swiss import counts, pixels


def compare(root, engine):
    observation = root / "observation.json"
    if (
        observation.exists()
        and json.loads(observation.read_text()).get("status") == "diagnostic-only"
    ):
        raise ValueError("diagnostic-only capture is not font-accuracy evidence")
    manifest = json.loads((root / "manifest.json").read_text())
    capture = json.loads((root / "zd621/capture.json").read_text())
    assert (
        capture["status"] == "complete"
        and capture["resident_repeat_exact"]
        and capture["font_selection_and_execution_verified"]
    )
    assert capture["cleanup"]["confirmed_absent"]
    assert capture["manifest_sha256"] == sha((root / "manifest.json").read_bytes())
    assert manifest["font_sha256"] == sha((root / "probe.ttf").read_bytes())
    queries = sorted(
        {
            (p["width"], p["height"], ord(c), "NRIB".index(p["orientation"]))
            for page in manifest["pages"]
            for p in page["probes"]
            for c in p["text"]
        }
    )
    result = subprocess.run(
        [str(engine.resolve()), "glyphs", str(root / "probe.ttf"), "zd621"],
        input="".join(" ".join(map(str, q)) + "\n" for q in queries),
        text=True,
        capture_output=True,
        check=True,
        timeout=120,
    )
    rows = dict(
        zip(queries, [json.loads(s) for s in result.stdout.splitlines()], strict=True)
    )
    report = dict(
        printer=capture["printer"],
        font_sha256=manifest["font_sha256"],
        manifest_sha256=capture["manifest_sha256"],
        pages=[],
    )
    for page in manifest["pages"]:
        record = next(p for p in capture["pages"] if p["name"] == page["name"])
        path = root / "zd621" / (page["name"] + ".png")
        assert sha(path.read_bytes()) == record["png_sha256"]
        assert (
            sha((root / "zd621" / (page["name"] + ".zpl")).read_bytes())
            == page["zpl_sha256"]
            == record["zpl_sha256"]
        )
        reference = ink(path, page["canvas"])
        candidate = set()
        cases = []
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            pen = 0
            points = set()
            turn = "NRIB".index(p["orientation"])
            for c in p["text"]:
                row = rows[(p["width"], p["height"], ord(c), turn)]
                if "error" in row:
                    raise ValueError((page["name"], p, row))
                dx, dy = [(pen, 0), (0, pen), (-pen, 0), (0, -pen)][turn]
                points.update(
                    (tx + ax + x + dx, ty + ay + y + dy) for x, y in pixels(row)
                )
                pen += row["layout_advance"]
            assert all(tx <= x < tx + tw and ty <= y < ty + th for x, y in points), (
                page["name"],
                p.get("name", p["text"]),
                "candidate escapes tile",
            )
            candidate.update(points)
            expected = {
                (x, y) for x, y in reference if tx <= x < tx + tw and ty <= y < ty + th
            }
            cases.append(
                dict(
                    name=p.get("name", p["text"]),
                    width=p["width"],
                    height=p["height"],
                    **counts(expected, points)
                )
            )
        report["pages"].append(
            dict(name=page["name"], cases=cases, **counts(reference, candidate))
        )
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = compare(args.root, args.engine)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print([(p["name"], p["under"], p["over"], p["xor"]) for p in result["pages"]])
