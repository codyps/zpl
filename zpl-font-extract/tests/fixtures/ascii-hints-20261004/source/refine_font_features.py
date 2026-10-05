"""Experimental additive local-extremum hint features for reconstructed fonts.

The original graph selects contour-wide extrema and long straight edges; inner
curve extrema can be absent. Preserve existing groups/programs, add explicit
on-curve local extrema, and admit new stem links only through observed ink.
Quadratic endpoint/control-point representation follows OpenType glyf:
https://learn.microsoft.com/en-us/typography/opentype/spec/glyf
No new outline points, size-specific instructions or bitmap strikes are added.
"""

import argparse
from copy import deepcopy
import json
from pathlib import Path
import statistics

import examine_font_accuracy as examination
import joint_hint_program as hints
import optimize_font as optimizer
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
from reconstruct_geometry import flatten, winding
from reconstruct_hints import clusters


def extend(state):
    result, additions = deepcopy(state), {}
    for char, shape in result["shapes"].items():
        points = [p for contour in shape for p in contour]
        loops = [flatten(c) for c in shape]
        additions[char] = []
        for axis, feature in enumerate(result["graph"][char]):
            groups, stems = feature["groups"], feature["stems"]
            old_count = len(groups)
            used = {i for g in groups for i in g["points"]}
            selected, offset = [], 0
            for contour in shape:
                n = len(contour)
                for i, p in enumerate(contour):
                    if not p[2] or offset + i in used:
                        continue
                    # Step over a horizontal/vertical plateau. Its two ends
                    # must see the same turning direction to be an extremum.
                    neighbors = [
                        next(
                            (
                                contour[(i + sign * j) % n][axis]
                                for j in range(1, n)
                                if contour[(i + sign * j) % n][axis] != p[axis]
                            ),
                            p[axis],
                        )
                        for sign in (-1, 1)
                    ]
                    if (p[axis] - neighbors[0]) * (p[axis] - neighbors[1]) > 0:
                        # Never modify a previously hinted group by merging
                        # new points into it. Nearby duplicate groups are left
                        # alone in this additive experiment.
                        if all(abs(p[axis] - g["value"]) > 4 for g in groups):
                            selected.append(offset + i)
                offset += n
            for cluster in clusters({points[i][axis] for i in selected}, 4):
                members = [i for i in selected if points[i][axis] in cluster]
                groups.append(
                    dict(
                        value=round(statistics.median(cluster)),
                        points=members,
                        span=[
                            min(points[i][1 - axis] for i in members),
                            max(points[i][1 - axis] for i in members),
                        ],
                    )
                )
            require(len(groups) <= 64, "feature graph exceeds compiler budget")
            added_stems = []
            for i, a in enumerate(groups):
                for j in range(i + 1, len(groups)):
                    if i < old_count and j < old_count:
                        continue
                    b = groups[j]
                    width = abs(b["value"] - a["value"])
                    lo, hi = max(a["span"][0], b["span"][0]), min(
                        a["span"][1], b["span"][1]
                    )
                    if not 32 <= width <= 512 or lo > hi:
                        continue
                    # Three positions across thickness and along support must
                    # stay in ink; a center-only sample can span a counter.
                    if all(
                        winding((v, s) if axis == 0 else (s, v), loops)
                        for t in (0.25, 0.5, 0.75)
                        for v in [a["value"] + (b["value"] - a["value"]) * t]
                        for u in (0.25, 0.5, 0.75)
                        for s in [lo + (hi - lo) * u]
                    ):
                        low, high = (i, j) if a["value"] < b["value"] else (j, i)
                        added_stems.append(
                            dict(
                                low=low, high=high, width=width, support=max(1, hi - lo)
                            )
                        )
            stems.extend(added_stems)
            result["programs"][char][axis].extend([None] * (len(groups) - old_count))
            additions[char].append(
                dict(axis=axis, groups=groups[old_count:], stems=added_stems)
            )
    require(
        hints.build(state) == hints.build(result), "additive graph changes initial font"
    )
    return result, additions


def run(args):
    model, digest, inputs, references, pages, train, check = examination.development(
        args.model, args.captures
    )
    seed, _ = pipeline.frozen(args.seed)
    require(
        sha((args.seed / "model.json").read_bytes()) == inputs["seed_sha256"],
        "seed differs",
    )
    original = hints.initialize(seed)
    baseline = model["state"]
    state, additions = extend(baseline)
    engine = pipeline.Engine(args.engine, args.cache, 10000)
    require(
        engine.digest == inputs["engine_sha256"], "feature experiment engine differs"
    )
    args.output.mkdir(parents=True, exist_ok=False)
    save(
        args.output / "plan.json",
        dict(
            model_sha256=digest,
            engine_sha256=engine.digest,
            scripts={
                name: sha(Path(__file__).with_name(name).read_bytes())
                for name in (
                    Path(__file__).name,
                    "examine_font_accuracy.py",
                    "joint_hint_program.py",
                    "optimize_font.py",
                    "reconstruct_font.py",
                    "reconstruct_geometry.py",
                    "reconstruct_hints.py",
                )
            },
            additions=additions,
            sweeps=2,
            neighborhood="all distinct valid one-edit neighbors",
            search_queries=sorted(train),
            audit_queries=sorted(check),
            selection="search only; old internal check scored once after freezing; no production promotion",
        ),
    )
    oracle = optimizer.Oracle(
        engine, {q: references[q] for q in sorted(train)}, original
    )
    stages = []
    for sweep in range(2):
        for char in sorted(state["shapes"]):
            best, score = state, oracle.score(state)
            seen = {json.dumps(state["programs"][char], sort_keys=True)}
            for program in hints.neighbors(state, char):
                key = json.dumps(program, sort_keys=True)
                if key in seen:
                    continue
                seen.add(key)
                try:
                    for axis, nodes in enumerate(program):
                        hints.ordered(nodes, state["graph"][char][axis])
                except ValueError:
                    continue
                candidate = deepcopy(state)
                candidate["programs"][char] = program
                measured = oracle.score(candidate)
                if measured < score - 1e-12:
                    best, score = candidate, measured
            state = best
            stage = dict(
                sweep=sweep,
                glyph=char,
                objective=score,
                group_errors=optimizer.means(oracle.errors(state)),
                evaluations=len(engine.seen),
            )
            stages.append(stage)
            save(args.output / "progress.json", stages)
            print(json.dumps(stage), flush=True)
    save(args.output / "candidate.json", state)
    data = hints.build(state)
    (args.output / "candidate.ttf").write_bytes(data)
    report = dict(
        schema="font-feature-experiment-v1",
        production_ready=False,
        model_sha256=digest,
        font_sha256=sha(data),
        plan_sha256=sha((args.output / "plan.json").read_bytes()),
        candidate_sha256=sha((args.output / "candidate.json").read_bytes()),
        variants={},
    )
    for label, candidate in (("baseline", baseline), ("extended_features", state)):
        rows = engine.render(hints.build(candidate), sorted(references))
        scored = pipeline.engine_api.score(pages, rows)
        cases = examination.report_cases(scored)
        keyed = {
            (
                c["width"] or c["height"],
                c["height"],
                ord(c["text"]),
                "NRIB".index(c["orientation"]),
            ): c
            for c in cases
        }
        report["variants"][label] = dict(
            search=examination.breakdown([keyed[q] for q in sorted(train)]),
            previous_internal_check=examination.breakdown(
                [keyed[q] for q in sorted(check)]
            ),
            pages=scored,
        )
    save(args.output / "report.json", report)
    print(json.dumps(dict(complete=True, objective=oracle.score(state))), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("seed", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--captures", nargs="+", type=Path, required=True)
    run(parser.parse_args())
