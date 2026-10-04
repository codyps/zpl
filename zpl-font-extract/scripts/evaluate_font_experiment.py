"""Audit predeclared font experiments against fresh native printer captures.

The audit plan pins font bytes, engine, excluded fitting queries and capture
manifests before capture. Evaluation reports every variant without selecting a
new winner. Each page must preserve baseline IoU and exact cases to pass.
"""

import argparse
from datetime import datetime
import json
from pathlib import Path

from examine_font_accuracy import breakdown, report_cases
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha


def evaluate(plan_path, fixtures, engine_path, cache, output):
    plan = json.loads(plan_path.read_text())
    require(plan["schema"] == "font-experiment-audit-v1", "unknown audit plan")
    require(
        sha(engine_path.read_bytes()) == plan["engine_sha256"], "audit engine differs"
    )

    # Resolve only fixture-relative locations recorded before the captures.
    def location(name):
        result = (fixtures / name).resolve()
        require(
            result.is_relative_to(fixtures.resolve()), "audit path escapes fixtures"
        )
        return result

    for artifact in plan["frozen_artifacts"]:
        require(
            sha(location(artifact["path"]).read_bytes()) == artifact["sha256"],
            "frozen experiment changed",
        )
    forbidden = set(map(tuple, plan["fitting_queries"]))
    selected, queries, provenance = [], set(), []
    for capture in plan["captures"]:
        root = location(capture["path"])
        require(
            sha((root / "manifest.json").read_bytes()) == capture["manifest_sha256"],
            "audit capture plan changed",
        )
        record = json.loads((root / "zd621/capture.json").read_text())
        require(
            datetime.fromisoformat(record["started_utc"])
            >= datetime.fromisoformat(plan["frozen_at"]),
            "audit capture predates experiment freeze",
        )
        pages, source = pipeline.load_pages(root, "validation")
        require(source["printer"] == plan["printer"], "audit printer differs")
        keys = set(pipeline.cases(pages))
        require(
            not (keys & (forbidden | queries)),
            "audit reuses fitting or duplicate queries",
        )
        queries.update(keys)
        selected.append((capture["path"], pages))
        provenance.append(dict(capture=capture["path"], **source))
    runner = pipeline.Engine(engine_path, cache, 32)
    report = dict(
        schema=plan["schema"],
        audit_plan_sha256=sha(plan_path.read_bytes()),
        provenance=provenance,
        variants={},
        production_ready=False,
    )
    baseline_pages = None
    require(plan["fonts"][0]["label"] == "baseline", "baseline must be evaluated first")
    for font in plan["fonts"]:
        data = location(font["path"]).read_bytes()
        require(sha(data) == font["sha256"], "audit font changed")
        rows = runner.render(data, sorted(queries))
        pages, cases = [], []
        for name, campaign in selected:
            scores = pipeline.engine_api.score(campaign, rows)
            cases.extend(report_cases(scores))
            pages.extend(dict(capture=name, **p) for p in scores)
        if baseline_pages is None:
            baseline_pages = pages
        passes = all(
            pipeline.totals([a])["iou"] >= pipeline.totals([b])["iou"]
            and pipeline.totals([a])["exact"] >= pipeline.totals([b])["exact"]
            for a, b in zip(pages, baseline_pages, strict=True)
        )
        result = dict(
            font_sha256=font["sha256"],
            passes_baseline_page_gate=passes,
            breakdown=breakdown(cases),
            pages=pages,
        )
        report["variants"][font["label"]] = result
        save(output, report)
        print(
            json.dumps(
                dict(
                    variant=font["label"],
                    gate=passes,
                    sizes=result["breakdown"]["sizes"],
                )
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path)
    parser.add_argument("fixtures", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    evaluate(args.plan, args.fixtures, args.engine, args.cache, args.output)
