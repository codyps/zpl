"""Fixed polygon initializer from 512-dot Font 0 silhouettes; no fitted hints.

Trace oriented pixel-cell edges and simplify at 0.65 source dots, the preexisting
font-fitting experiment's tolerance. Render that one outline at other sizes with
the original TrueType engine. Never align, rescale, or trim reference canvases.
This assesses geometric information, not original bytecode recovery or spacing.
"""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import subprocess
import tempfile

from analyze_font_probe import ink
from capture_font_probe import sha
from compare_swiss import counts, pixels
import font_probe


def trace(points):
    edges = set()
    for x, y in points:
        corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
        for a, b in zip(corners, corners[1:] + corners[:1]):
            if (b, a) in edges:
                edges.remove((b, a))
            else:
                edges.add((a, b))
    outgoing = defaultdict(set)
    for a, b in edges:
        outgoing[a].add(b)
    loops = []
    while edges:
        start, second = min(edges)
        loop = [start]
        previous, current = start, second
        edges.remove((start, second))
        outgoing[start].remove(second)
        while current != start:
            loop.append(current)
            options = outgoing[current]
            assert options, "open contour"
            dx, dy = current[0] - previous[0], current[1] - previous[1]
            # At a diagonal contact turn right to keep separate ink components.
            order = [
                (current[0] - dy, current[1] + dx),
                (current[0] + dx, current[1] + dy),
                (current[0] + dy, current[1] - dx),
            ]
            next_point = next((p for p in order if p in options), None)
            assert next_point is not None
            edges.remove((current, next_point))
            options.remove(next_point)
            previous, current = current, next_point
        loops.append(loop)
    return loops


def simplify(points):
    a, b = points[0], points[-1]
    dx, dy = b[0] - a[0], b[1] - a[1]
    den = dx * dx + dy * dy
    distances = []
    for p in points[1:-1]:
        t = (
            max(0, min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / den))
            if den
            else 0
        )
        distances.append((p[0] - a[0] - t * dx) ** 2 + (p[1] - a[1] - t * dy) ** 2)
    if not distances or max(distances) <= 0.65**2:
        return [a, b]
    i = distances.index(max(distances)) + 1
    return simplify(points[: i + 1])[:-1] + simplify(points[i:])


def contours(points):
    result = []
    for loop in trace(points):
        i = max(
            range(len(loop)),
            key=lambda i: (loop[i][0] - loop[0][0]) ** 2
            + (loop[i][1] - loop[0][1]) ** 2,
        )
        result.append(
            simplify(loop[: i + 1])[:-1] + simplify(loop[i:] + [loop[0]])[:-1]
        )
    return result


def analyze(root, engine):
    manifest = json.loads((root / "manifest.json").read_text())
    capture = json.loads((root / "zd621/capture.json").read_text())
    assert (
        capture["status"] == "complete"
        and capture["resident_repeat_exact"]
        and capture["outline_repeat_exact"]
    )
    assert sha((root / "manifest.json").read_bytes()) == capture["manifest_sha256"]

    def reference(page):
        record = next(p for p in capture["pages"] if p["name"] == page["name"])
        data = (root / "zd621" / (page["name"] + ".png")).read_bytes()
        assert sha(data) == record["png_sha256"]
        assert (
            sha((root / "zd621" / (page["name"] + ".zpl")).read_bytes())
            == record["zpl_sha256"]
            == page["zpl_sha256"]
        )
        return ink(root / "zd621" / (page["name"] + ".png"), page["canvas"])

    models = {}
    summary = []
    for page in manifest["pages"]:
        if page["probes"][0]["height"] != 512:
            continue
        points = reference(page)
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            selected = {
                (x, y) for x, y in points if tx <= x < tx + tw and ty <= y < ty + th
            }
            assert selected and all(
                tx + 1 < x < tx + tw - 2 and ty + 1 < y < ty + th - 2
                for x, y in selected
            ), "clipped silhouette"
            shape = contours({(x - ax, y - ay) for x, y in selected})
            models[p["text"]] = [
                [(x * 4, -y * 4, True) for x, y in contour] for contour in shape
            ]
            summary.append(
                dict(
                    text=p["text"],
                    ink=len(selected),
                    contours=len(shape),
                    vertices=sum(map(len, shape)),
                )
            )
    # The probe builder maps contiguous characters starting at U+0020.
    items = [
        dict(
            name=f"outline-{code}",
            codepoint=code,
            contours=models.get(chr(code), []),
            instructions="",
            advance=2048,
            family="pilot",
        )
        for code in range(32, max(map(ord, models)) + 1)
    ]
    data = font_probe.font(items)
    queries = sorted(
        {
            (p["height"], p["height"], ord(p["text"]), 0)
            for page in manifest["pages"]
            for p in page["probes"]
        }
    )
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "pilot.ttf"
        path.write_bytes(data)
        output = subprocess.run(
            [str(engine.resolve()), "glyphs", str(path), "center", "none"],
            input="".join(" ".join(map(str, q)) + "\n" for q in queries),
            text=True,
            capture_output=True,
            check=True,
            timeout=120,
        )
    rows = dict(
        zip(queries, [json.loads(s) for s in output.stdout.splitlines()], strict=True)
    )
    report = dict(
        schema="font0-outline-initializer-v1",
        printer=capture["printer"],
        manifest_sha256=capture["manifest_sha256"],
        source_size=512,
        simplification_tolerance=0.65,
        spacing="not measured; isolated geometry only",
        models=summary,
        pages=[],
    )
    for page in manifest["pages"]:
        expected = reference(page)
        candidate = set()
        for p in page["probes"]:
            row = rows[(p["height"], p["height"], ord(p["text"]), 0)]
            assert "error" not in row, row
            ax, ay = p["anchor"]
            tx, ty, tw, th = p["tile"]
            points = {(x + ax, y + ay) for x, y in pixels(row)}
            assert all(tx <= x < tx + tw and ty <= y < ty + th for x, y in points)
            candidate.update(points)
        stats = counts(expected, candidate)
        report["pages"].append(
            dict(
                name=page["name"],
                group=page["group"],
                **stats,
                iou=1 - stats["xor"] / stats["union"],
            )
        )
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = analyze(args.root, args.engine)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
