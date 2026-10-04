"""Compare frozen proposals on identical native captures, including size strata.

Consumes reconstruction evaluation reports; no pixels, fonts or fitted models
are changed. Per-case sums must account for every pixel in each native canvas.
"""

import argparse
import json
from pathlib import Path

from reconstruct_font import require, save


def summarize(cases):
    result = {k: sum(c[k] for c in cases) for k in ("under", "over", "xor", "union")}
    result["iou"] = 1 - result["xor"] / result["union"] if result["union"] else 1
    result["cases"] = len(cases)
    result["exact"] = sum(c["xor"] == 0 for c in cases)
    result["mean_error"] = sum(
        c["xor"] / c["union"] if c["union"] else 0 for c in cases
    ) / len(cases)
    return result


def stratum(case):
    w, h = case["width"] or case["height"], case["height"]
    if min(w, h) >= 128:
        return "large"
    return next(
        name
        for limit, name in (
            (32, "up_to_32"),
            (64, "33_to_64"),
            (90, "65_to_90"),
            (4096, "91_plus"),
        )
        if max(w, h) <= limit
    )


def compare(previous, current):
    require(
        previous["group"] == current["group"] == "validation",
        "comparison requires validation reports",
    )
    old = {c["capture"]: c for c in previous["campaigns"]}
    new = {c["capture"]: c for c in current["campaigns"]}
    require(
        len(old) == len(previous["campaigns"])
        and len(new) == len(current["campaigns"]),
        "duplicate comparison campaign",
    )
    require(old.keys() == new.keys(), "comparison campaigns differ")
    result = dict(
        previous_model_sha256=previous["model_sha256"],
        current_model_sha256=current["model_sha256"],
        campaigns=[],
        strata={},
    )
    pooled = {label: [] for label in ("seed", "previous_proposal", "expanded_proposal")}
    for name, campaign in new.items():
        require(
            campaign["provenance"] == old[name]["provenance"],
            "comparison captures differ",
        )
        require(
            campaign["fonts"]["initial.ttf"] == old[name]["fonts"]["initial.ttf"],
            "comparison seeds differ",
        )
        row = dict(capture=name, totals={})
        for label, pages in (
            ("seed", campaign["fonts"]["initial.ttf"]),
            ("previous_proposal", old[name]["fonts"]["proposal.ttf"]),
            ("expanded_proposal", campaign["fonts"]["proposal.ttf"]),
        ):
            cases = []
            for page in pages:
                for key in ("under", "over", "xor", "union"):
                    require(
                        sum(c[key] for c in page["cases"]) == page[key],
                        "case counts do not cover native canvas",
                    )
                cases.extend(page["cases"])
            row["totals"][label] = summarize(cases)
            pooled[label].extend(cases)
        row["passes_seed_gate"] = campaign["passes_proposal_outline_gate"]
        result["campaigns"].append(row)
    for band in sorted({stratum(c) for c in pooled["seed"]}):
        result["strata"][band] = {
            label: summarize([c for c in cases if stratum(c) == band])
            for label, cases in pooled.items()
        }
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("previous", type=Path)
    parser.add_argument("current", type=Path)
    parser.add_argument("--previous-extra", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    previous = json.loads(args.previous.read_text())
    for path in args.previous_extra:
        extra = json.loads(path.read_text())
        require(
            extra["model_sha256"] == previous["model_sha256"]
            and extra["group"] == previous["group"],
            "previous reports differ in model or group",
        )
        previous["campaigns"].extend(extra["campaigns"])
    report = compare(previous, json.loads(args.current.read_text()))
    save(args.output, report)
    print(json.dumps(report["strata"], indent=2))
