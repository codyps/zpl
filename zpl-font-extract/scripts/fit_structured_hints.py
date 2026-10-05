"""Compare hint structure and regression-aware objectives on frozen search data.

Four predeclared arms cross original/relational structures and legacy/robust
objectives. Outlines, CVTs, the 90-ppem outer guard and the development split stay
fixed. The old internal check is an audit only, scored after all fonts freeze.
"""

import argparse
from collections import defaultdict
from copy import deepcopy
import json
import math
from pathlib import Path

import examine_font_accuracy as examination
import joint_hint_program as hints
import optimize_font as legacy
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
import structured_font_hints as structure


def band(query):
    x, y, _, _ = query
    if min(x, y) >= 128:
        return "large"
    return next(
        str(limit)
        for limit in (16, 24, 32, 48, 64, 90, 128, 4096)
        if max(x, y) <= limit
    )


def error(stats):
    return stats["xor"] / stats["union"] if stats["union"] else 0.0


def aggregate(stats):
    values = list(stats)
    return (
        sum(error(v) for v in values) / len(values),
        sum(v["xor"] for v in values) / max(1, sum(v["union"] for v in values)),
    )


def projection_error(reference, candidate, axis):
    """Foreground multiset IoU after projection onto a native canvas axis.

    Used only to nominate coordinated hint edits. A translation on the other
    axis leaves this guide invariant; accepted scores still use every 2D pixel.
    """
    a, b = defaultdict(int), defaultdict(int)
    for point in reference:
        a[point[axis]] += 1
    for point in candidate:
        b[point[axis]] += 1
    coordinates = a.keys() | b.keys()
    union = sum(max(a[x], b[x]) for x in coordinates)
    return sum(abs(a[x] - b[x]) for x in coordinates) / union if union else 0


class Objective:
    def __init__(self, baseline, mode):
        self.baseline, self.mode = baseline, mode
        self.buckets = defaultdict(list)
        self.transforms = defaultdict(list)
        for q in baseline:
            self.buckets[(q[2], band(q))].append(q)
            kind = (
                "rotated"
                if q[3]
                else "square" if q[0] == q[1] else "wide" if q[0] > q[1] else "tall"
            )
            self.transforms[(q[2], kind)].append(q)
        self.floors = {
            key: aggregate(baseline[q] for q in keys)
            for key, keys in self.buckets.items()
        }

    def measure(self, stats, cost=0):
        errors = {q: error(v) for q, v in stats.items()}
        violations = []
        if any(
            error(v) > error(self.baseline[q]) + 1e-12
            for q, v in stats.items()
            if band(q) == "large"
        ):
            violations.append("large")
        if self.mode == "legacy":
            groups = legacy.means(errors)
            return (
                sum(groups.values()) / len(groups)
                + 0.2 * max(groups.values())
                + 5e-5 * cost,
                violations,
            )
        if any(v["xor"] for q, v in stats.items() if not self.baseline[q]["xor"]):
            violations.append("exact")
        if any(errors[q] - error(self.baseline[q]) > 0.06 + 1e-12 for q in stats):
            violations.append("case-regret")
        values = []
        for key, keys in self.buckets.items():
            current = aggregate(stats[q] for q in keys)
            if any(
                a > b + 1e-12 for a, b in zip(current, self.floors[key], strict=True)
            ):
                violations.append("glyph-size")
            values.append(sum(current) / 2)
        for keys in self.transforms.values():
            if any(
                a > b + 1e-12
                for a, b in zip(
                    aggregate(stats[q] for q in keys),
                    aggregate(self.baseline[q] for q in keys),
                    strict=True,
                )
            ):
                violations.append("glyph-transform")
        # Equal weight for each glyph/size band, both macro and pooled pixel
        # error, an upper-quartile tail and a penalty for casewise regret.
        tail = sorted(values, reverse=True)[: max(1, math.ceil(len(values) / 4))]
        regret = sum(max(0, errors[q] - error(self.baseline[q])) for q in stats) / len(
            stats
        )
        score = (
            sum(values) / len(values)
            + 0.25 * sum(tail) / len(tail)
            + 2 * regret
            + 5e-5 * cost
        )
        return score, sorted(set(violations))


class Oracle:
    """Exact-count cache keyed by the programs active at each projected ppem.

    Every cache miss executes the actual complete TTF. A final uncached whole-font
    audit verifies all search/check counts. No approximation determines a score.
    Shapes and CVTs must be identical to the baseline throughout this experiment.
    """

    def __init__(self, engine, references, baseline, projections=False):
        self.engine, self.references, self.baseline = engine, references, baseline
        self.memo = {}
        self.projections = {} if projections else None
        self.last_projections = {}
        self.keys = {
            c: sorted(q for q in references if q[2] == ord(c))
            for c in baseline["shapes"]
        }
        self.floor = self.stats(baseline)

    def stats(self, state, queries=None):
        require(
            state["shapes"] == self.baseline["shapes"]
            and state["parameters"] == self.baseline["parameters"],
            "structured search changed fixed geometry or CVTs",
        )
        result = {}
        self.last_projections = {}
        selected = set(queries) if queries is not None else None
        for char, all_keys in self.keys.items():
            keys = (
                all_keys if selected is None else [q for q in all_keys if q in selected]
            )
            regimes = state.get("regimes", {}).get(char, [None, None])
            optical = state.get("optical_programs", {}).get(char)
            identity = {}
            cachekeys = {}
            for q in keys:
                projected = [math.floor(pipeline.ppem(d) + 0.5) for d in q[:2]]
                active = (
                    tuple(
                        bool(r and hints.regime_matches(r, a, projected))
                        for a, r in enumerate(regimes)
                    )
                    if max(projected) <= state["parameters"]["limit"]
                    else None
                )
                if char in state.get("independent_axes", []):
                    active = tuple(
                        (
                            None
                            if projected[a] > state["parameters"]["limit"]
                            else bool(r and hints.regime_matches(r, a, projected))
                        )
                        for a, r in enumerate(regimes)
                    )
                if active is not None:
                    active = tuple(
                        (
                            2
                            if flag
                            and regimes[a].get("smaller")
                            and hints.regime_matches(
                                regimes[a]["smaller"], a, projected
                            )
                            else flag
                        )
                        for a, flag in enumerate(active)
                    )
                    if optical:
                        active = tuple(
                            (
                                3
                                if flag is not None
                                and optical["nodes"][a] is not None
                                and hints.regime_matches(
                                    dict(
                                        limit=optical["limit"],
                                        measure=optical.get("measures", ["max", "max"])[
                                            a
                                        ],
                                    ),
                                    a,
                                    projected,
                                )
                                else flag
                            )
                            for a, flag in enumerate(active)
                        )
                if active not in identity:
                    programs = (
                        [
                            (
                                None
                                if active[a] is None
                                else (
                                    optical["nodes"][a]
                                    if active[a] == 3
                                    else (
                                        regimes[a]["smaller"]["nodes"]
                                        if active[a] == 2
                                        else (
                                            regimes[a]["nodes"]
                                            if active[a]
                                            else state["programs"][char][a]
                                        )
                                    )
                                )
                            )
                            for a in (0, 1)
                        ]
                        if active is not None
                        else None
                    )
                    identity[active] = sha(
                        json.dumps(
                            [
                                programs,
                                state["graph"][char][0]["groups"],
                                state["graph"][char][1]["groups"],
                                state["zone_origins"],
                            ],
                            sort_keys=True,
                        ).encode()
                    )
                diagonal = state.get("diagonal_programs", {}).get(char)
                diagonal_active = (
                    diagonal
                    if diagonal
                    and min(projected) > diagonal["start"]
                    and max(projected)
                    <= min(diagonal["limit"], state["parameters"]["limit"])
                    else None
                )
                cachekeys[q] = (
                    identity[active],
                    json.dumps(diagonal_active, sort_keys=True),
                    q,
                )
            missing = [q for q in keys if cachekeys[q] not in self.memo]
            if missing:
                rows = self.engine.render(hints.build(state, [char]), missing)
                for q in missing:
                    pixels = pipeline.pixels(rows[q])
                    self.memo[cachekeys[q]] = pipeline.counts(
                        self.references[q], pixels
                    )
                    if self.projections is not None:
                        self.projections[cachekeys[q]] = (
                            [
                                projection_error(
                                    self.references[q], pixels, (axis + q[3]) % 2
                                )
                                for axis in (0, 1)
                            ],
                            sha(json.dumps(sorted(pixels)).encode()),
                        )
            result.update((q, self.memo[cachekeys[q]]) for q in keys)
            if self.projections is not None:
                self.last_projections.update(
                    (q, self.projections[cachekeys[q]]) for q in keys
                )
        return result


def fit(args):
    model, digest, inputs, references, pages, train, check = examination.development(
        args.model, args.captures
    )
    baseline = model["state"]
    args.output.mkdir(parents=True, exist_ok=False)
    engine = pipeline.Engine(args.engine, args.cache, 30000)
    require(engine.digest == inputs["engine_sha256"], "fitting engine differs")
    plan = dict(
        schema="structured-font-fit-v1",
        model_sha256=digest,
        engine_sha256=engine.digest,
        printer=inputs["source"]["printer"],
        search_queries=sorted(train),
        internal_check_queries=sorted(check),
        arms=[
            "original_legacy",
            "original_robust",
            "structured_legacy",
            "structured_robust",
        ],
        sweeps=2,
        neighbors_per_axis=512,
        cutoffs=structure.CUTOFFS,
        objective=dict(
            macro_weight=0.5,
            pooled_weight=0.5,
            tail_fraction=0.25,
            tail_weight=0.25,
            regret_weight=2,
            complexity=5e-5,
            max_case_regression=0.06,
            guards=[
                "every large case",
                "every exact case",
                "glyph by size mean and pooled error",
                "glyph by transform mean and pooled error",
            ],
        ),
        scripts={
            n: sha(Path(__file__).with_name(n).read_bytes())
            for n in (
                Path(__file__).name,
                "structured_font_hints.py",
                "joint_hint_program.py",
                "refine_font_features.py",
                "examine_font_accuracy.py",
                "reconstruct_font.py",
                "optimize_font.py",
            )
        },
        selection="two deterministic sweeps on frozen search only; robust structured candidate designated before old-check/fresh audits; no reselection using audit pixels",
        production_ready=False,
    )
    save(args.output / "plan.json", plan)
    oracle = Oracle(engine, {q: references[q] for q in sorted(train)}, baseline)
    candidates = {"baseline": deepcopy(baseline)}
    progress = []
    for label in plan["arms"]:
        structured = label.startswith("structured")
        objective = Objective(oracle.floor, label.split("_")[1])
        state = structure.initialize(baseline) if structured else deepcopy(baseline)

        def score(s):
            value, violations = objective.measure(
                oracle.stats(s), structure.complexity(s, baseline)
            )
            return float("inf") if violations else value

        for sweep in range(plan["sweeps"]):
            for char in sorted(state["shapes"]):
                for axis in (0, 1):
                    before = score(state)
                    best, best_score = state, before
                    choices = {
                        json.dumps(
                            [
                                s["programs"][char][axis],
                                s.get("regimes", {}).get(char, [None, None])[axis],
                            ],
                            sort_keys=True,
                        ): s
                        for s in structure.neighbors(state, char, axis, structured)
                    }
                    keys = sorted(
                        choices,
                        key=lambda k: sha(f"{sweep}:{char}:{axis}:{k}".encode()),
                    )[: plan["neighbors_per_axis"]]
                    rejected = defaultdict(int)
                    for key in keys:
                        candidate = choices[key]
                        value, violations = objective.measure(
                            oracle.stats(candidate),
                            structure.complexity(candidate, baseline),
                        )
                        for v in violations:
                            rejected[v] += 1
                        if not violations and value < best_score - 1e-12:
                            best, best_score = candidate, value
                    state = best
                    row = dict(
                        arm=label,
                        sweep=sweep,
                        glyph=char,
                        axis=axis,
                        before=before,
                        after=best_score,
                        proposals=len(choices),
                        scored=len(keys),
                        rejections=dict(rejected),
                        edits=structure.complexity(state, baseline),
                        evaluations=len(engine.seen),
                    )
                    progress.append(row)
                    save(args.output / "progress.json", progress)
                    print(json.dumps(row), flush=True)
        candidates[label] = state
    save(args.output / "candidates.json", candidates)
    for label, state in candidates.items():
        (args.output / (label + ".ttf")).write_bytes(hints.build(state))
    report = dict(
        schema=plan["schema"],
        plan_sha256=sha((args.output / "plan.json").read_bytes()),
        candidates_sha256=sha((args.output / "candidates.json").read_bytes()),
        designated="structured_robust",
        production_ready=False,
        variants={},
    )
    for label, state in candidates.items():
        data = hints.build(state)
        rows = engine.render(data, sorted(references))
        scored = pipeline.engine_api.score(pages, rows)
        cs = examination.report_cases(scored)
        keyed = {
            (
                c["width"] or c["height"],
                c["height"],
                ord(c["text"]),
                "NRIB".index(c["orientation"]),
            ): c
            for c in cs
        }
        cached = oracle.stats(state)
        require(
            all(
                all(keyed[q][k] == v[k] for k in ("under", "over", "xor", "union"))
                for q, v in cached.items()
            ),
            "active-program cache disagrees with whole-font execution",
        )
        report["variants"][label] = dict(
            font_sha256=sha(data),
            search=examination.breakdown([keyed[q] for q in sorted(train)]),
            previous_internal_check=examination.breakdown(
                [keyed[q] for q in sorted(check)]
            ),
            pages=scored,
            cache_audit_exact=True,
        )
        save(args.output / "report.json", report)
        print(
            json.dumps(
                dict(
                    evaluated=label,
                    search=report["variants"][label]["search"]["sizes"],
                    check=report["variants"][label]["previous_internal_check"]["sizes"],
                )
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--captures", type=Path, nargs="+", required=True)
    fit(parser.parse_args())
