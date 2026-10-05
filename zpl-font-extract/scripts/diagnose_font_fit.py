"""Explain weak reconstruction cases without fitting or registering rasters.

Reports native pixel counts, bounds, centroids and unhinted-outline controls.
Worst-case selection uses the existing training partition; the old internal
check is explicitly separate. Nothing translates or rescales measured pixels.
"""

import argparse
from copy import deepcopy
import json
from pathlib import Path

from fit_font_target import metrics
from fit_structured_hints import error
import joint_hint_program as hints
import reconstruct_ascii_font as ascii_fit
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
from repair_font_search import development


def footprint(pixels):
    if not pixels:
        return dict(ink=0, bounds=None, centroid=None)
    return dict(
        ink=len(pixels),
        bounds=[min(p[a] for p in pixels) for a in (0, 1)]
        + [max(p[a] for p in pixels) + 1 for a in (0, 1)],
        centroid=[sum(p[a] for p in pixels) / len(pixels) for a in (0, 1)],
    )


def diagnose(fixtures, fit, engine, cache, output, count):
    plan_data, report_data, state_data = (
        (fit / name).read_bytes()
        for name in ("plan.json", "report.json", "candidates.json")
    )
    plan, report, states = map(json.loads, (plan_data, report_data, state_data))
    require(
        sha(plan_data) == report["plan_sha256"]
        and sha(state_data) == report["candidates_sha256"],
        "fit changed",
    )
    pages, provenance = development(fixtures)
    references = pipeline.cases(pages)
    queries = sorted(references)
    runner = pipeline.Engine(engine, cache, 4)
    controls = dict(baseline=states["baseline"], repaired=states["targeted"])
    for label, variant in (("baseline", "baseline"), ("repaired", "targeted")):
        require(
            sha(hints.build(controls[label]))
            == report["variants"][variant]["font_sha256"],
            "compiled diagnostic font differs from frozen fit",
        )
    unhinted = deepcopy(states["baseline"])
    for axes in unhinted["programs"].values():
        for nodes in axes:
            nodes[:] = [None] * len(nodes)
    for key in ("regimes", "optical_programs", "diagonal_programs", "independent_axes"):
        unhinted.pop(key, None)
    controls["unhinted"] = unhinted
    rows = {k: runner.render(hints.build(v), queries) for k, v in controls.items()}
    stats = {k: ascii_fit.score(v, references) for k, v in rows.items()}
    result = dict(
        schema="font-fit-diagnosis-v1",
        engine_sha256=sha(engine.read_bytes()),
        candidates_sha256=sha(state_data),
        captures=provenance,
        partitions={},
    )
    for partition, field in (
        ("training", "search_queries"),
        ("previously_observed_check", "internal_check_queries"),
    ):
        keys = set(map(tuple, plan[field]))
        require(keys <= references.keys(), "partition has unavailable cases")
        for label, variant in (("baseline", "baseline"), ("repaired", "targeted")):
            require(
                metrics({q: stats[label][q] for q in keys})
                == report["variants"][variant][
                    "search_target" if partition == "training" else "check_target"
                ],
                "diagnostic execution differs from frozen fit metrics",
            )
        worst = sorted(keys, key=lambda q: (-error(stats["baseline"][q]), q))[:count]
        result["partitions"][partition] = dict(
            metrics={k: metrics({q: s[q] for q in keys}) for k, s in stats.items()},
            worst=[
                dict(
                    query=q,
                    glyph=chr(q[2]),
                    reference=footprint(references[q]),
                    variants={
                        k: dict(**stats[k][q], **footprint(pipeline.pixels(rows[k][q])))
                        for k in controls
                    },
                )
                for q in worst
            ],
        )
    save(output, result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("fixtures", "fit", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--count", type=int, default=24)
    args = parser.parse_args()
    require(1 <= args.count <= 100, "diagnostic case budget exceeded")
    diagnose(args.fixtures, args.fit, args.engine, args.cache, args.output, args.count)
