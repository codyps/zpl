"""Fit bounded point relationships, never per-size correction tables.

The only optimization inputs are development pages. Each point group selects
one TrueType constraint: untouched, grid-snapped, zone-snapped, linked to another
feature, or a centered stem. Glyph geometry remains the 512-dot initializer.
"""

import argparse
import json
from pathlib import Path

import font0_hint_model as m
from compare_swiss import pixels, counts
from capture_font_probe import sha


def fit(source, targets, engine, output, passes=2):
    shapes = m.load_models(source)
    pages = m.load_pages(targets, "0", "development")
    params = dict(
        stem_x=272, stem_y=208, cutin=24, bias_x=0, bias_y=0, overshoot=36, edge_min=80
    )
    hints = {
        c: [["none"] * len(m.axis_groups(shape, a, params["edge_min"])) for a in [0, 1]]
        for c, shape in shapes.items()
    }
    history = []
    for c, shape in shapes.items():
        cases = []
        for page, ref in pages:
            for p in page["probes"]:
                if p["text"] != c:
                    continue
                tx, ty, tw, th = p["tile"]
                ax, ay = p["anchor"]
                expected = {
                    (x - tx - ax, y - ty - ay)
                    for x, y in ref
                    if tx <= x < tx + tw and ty <= y < ty + th
                }
                cases.append(
                    (
                        (
                            p["width"],
                            p["height"],
                            ord(c),
                            "NRIB".index(p["orientation"]),
                        ),
                        expected,
                    )
                )
        qs = sorted({q for q, _ in cases})
        cache = {}

        def loss():
            key = tuple(tuple(a) for a in hints[c])
            if key in cache:
                return cache[key]
            try:
                data = m.compile_hints({c: shape}, {c: hints[c]}, params)
            except ValueError:
                return (float("inf"), 0)
            rows = m.engine_rows(engine, data, qs)
            error = 0
            for q, ref in cases:
                stats = counts(ref, pixels(rows[q]))
                error += stats["xor"] / stats["union"] if stats["union"] else 0
            result = (
                error,
                sum(
                    (
                        2
                        if mode in ("grid", "zone", "center_prev", "center_next")
                        else 1 if mode != "none" else 0
                    )
                    for axis in hints[c]
                    for mode in axis
                ),
            )
            cache[key] = result
            return result

        best = loss()
        initial = best
        for iteration in range(passes):
            previous = best
            for axis in (0, 1):
                groups = m.axis_groups(shape, axis, params["edge_min"])
                for i, group in enumerate(groups):
                    selected = hints[c][axis][i]
                    for mode in [
                        "none",
                        "grid",
                        "zone",
                        "prev",
                        "next",
                        "center_prev",
                        "center_next",
                    ] + [f"link:{j}" for j in range(len(groups)) if abs(j - i) > 1]:
                        if "prev" in mode and i == 0:
                            continue
                        if "next" in mode and i == len(groups) - 1:
                            continue
                        if mode == "zone" and (
                            axis != 1
                            or min(abs(group["value"] - z) for z in [0, 1144, 1532])
                            > 64
                        ):
                            continue
                        hints[c][axis][i] = mode
                        candidate = loss()
                        if candidate < best:
                            best = candidate
                            selected = mode
                    hints[c][axis][i] = selected
            # Joint moves can establish a stem whose two endpoints each fail
            # when fitted alone. These are fixed point relationships at all sizes.
            for axis in (0, 1):
                groups = m.axis_groups(shape, axis, params["edge_min"])
                for i in range(len(groups) - 1):
                    selected = hints[c][axis][i : i + 2]
                    for pair in [
                        ("none", "none"),
                        ("grid", "grid"),
                        ("grid", "prev"),
                        ("next", "grid"),
                        ("center_next", "prev"),
                        ("next", "center_prev"),
                        ("zone", "prev"),
                        ("next", "zone"),
                    ]:
                        if "zone" in pair:
                            zi = i + pair.index("zone")
                            if (
                                axis != 1
                                or min(
                                    abs(groups[zi]["value"] - z)
                                    for z in [0, 1144, 1532]
                                )
                                > 64
                            ):
                                continue
                        hints[c][axis][i : i + 2] = pair
                        candidate = loss()
                        if candidate < best:
                            best = candidate
                            selected = list(pair)
                    hints[c][axis][i : i + 2] = selected
            print(c, "pass", iteration + 1, "loss", best, "from", initial, flush=True)
            history.append(
                dict(text=c, iteration=iteration + 1, loss=best[0], constraints=best[1])
            )
            if best == previous:
                break
    model = dict(
        schema="font0-hint-graph-v1",
        source_manifest_sha256=sha((source / "manifest.json").read_bytes()),
        development_manifest_sha256=sha((targets / "manifest.json").read_bytes()),
        parameters=params,
        hints=hints,
        history=history,
    )
    output.write_text(json.dumps(model, indent=2) + "\n")
    data = m.compile_hints(shapes, hints, params)
    output.with_suffix(".ttf").write_bytes(data)
    report = m.score(pages, m.engine_rows(engine, data, m.queries_for(pages)))
    output.with_suffix(".development.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    print([(p["name"], p["xor"], p["iou"]) for p in report], flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("targets", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--passes", type=int, choices=range(1, 5), default=2)
    args = parser.parse_args()
    fit(args.source, args.targets, args.engine, args.output, args.passes)
