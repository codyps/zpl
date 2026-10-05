"""Repair the weakest training glyphs with bounded, non-greedy hint search.

Keep an admissible incumbent separate from exploratory programs. Temporary
regressions may nominate a coordinated repair, but cannot become output. Search
uses only the frozen training split. Outlines, CVTs and advance metrics stay fixed.
The existing compiler emits SCFS/IP/IUP TrueType instructions as specified at
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor, as_completed
from copy import deepcopy
import json
from pathlib import Path

from capture_font_probe import now
import examine_font_accuracy as examination
from fit_ascii_hints import one_glyph, replace_glyph, neighborhood
from fit_font_target import copy_hint_state, metrics, proposals, target_score
from fit_structured_hints import Objective, Oracle, aggregate, error
import joint_hint_program as hints
import reconstruct_ascii_font as ascii_fit
import reconstruct_font as pipeline
import projection_hint_search
import diagonal_font_hints
from reconstruct_font import require, save, sha


def axis_programs(state, char, axis):
    yield state["programs"][char][axis]
    regime = state.get("regimes", {}).get(char, [None, None])[axis]
    while regime:
        yield regime["nodes"]
        regime = regime.get("smaller")
    optical = state.get("optical_programs", {}).get(char)
    if optical and optical["nodes"][axis] is not None:
        yield optical["nodes"][axis]


def complete_axes(state):
    """Expose a geometric anchor on both axes without changing its initial hint.

    The old shoulder selector stops at the union of X and Y anchors. Consequently
    a diagonal corner already controlled in X can be completely absent from the
    Y graph. Borrow those existing geometric anchors, preserving every old group
    and reference index. This reads outlines, never captured pixels.
    """
    result = deepcopy(state)
    for char, shape in result["shapes"].items():
        points = [p for contour in shape for p in contour]
        anchors = sorted(
            {
                i
                for axis in state["graph"][char]
                for group in axis["groups"]
                for i in group["points"]
            }
        )
        for axis, feature in enumerate(result["graph"][char]):
            used = {i for group in feature["groups"] for i in group["points"]}
            for i in anchors:
                if i in used or len(feature["groups"]) >= 64:
                    continue
                # Equal coordinates remain separate controls: they can belong
                # to disconnected contour runs with different optical behavior.
                feature["groups"].append(
                    dict(
                        points=[i],
                        value=points[i][axis],
                        span=[points[i][1 - axis]] * 2,
                        cross_axis=True,
                    )
                )
                for nodes in axis_programs(result, char, axis):
                    nodes.append(None)
    require(
        hints.build(result) == hints.build(state), "axis completion changed the seed"
    )
    return result


class Constraints:
    """Retain both historical guards and improvements already in the incumbent."""

    def __init__(self, original, incumbent):
        self.objectives = [
            Objective(original, "robust"),
            Objective(incumbent, "robust"),
        ]
        self.buckets = self.objectives[0].buckets

    def measure(self, stats):
        violations = []
        for i, objective in enumerate(self.objectives):
            violations += [f"{i}:{v}" for v in objective.measure(stats)[1]]
        return 0, violations

    def excess(self, stats):
        """Continuous guard excess guides repair; acceptance still uses measure."""
        residuals = []
        for objective in self.objectives:
            for q, s in stats.items():
                limit = error(objective.baseline[q])
                if min(q[:2]) < 128 and objective.baseline[q]["xor"]:
                    limit += 0.06
                residuals.append(max(0, error(s) - limit))
            for groups in (objective.buckets, objective.transforms):
                for keys in groups.values():
                    current = aggregate(stats[q] for q in keys)
                    previous = aggregate(objective.baseline[q] for q in keys)
                    residuals.extend(
                        max(0, a - b) for a, b in zip(current, previous, strict=True)
                    )
        return max(residuals, default=0), sum(v * v for v in residuals)


class Relaxed:
    def __init__(self, constraints):
        self.buckets = constraints.buckets

    def measure(self, stats):
        return 0, []


def identity(state, char):
    return sha(
        json.dumps(
            [
                state["programs"][char],
                state.get("regimes", {}).get(char),
                state.get("optical_programs", {}).get(char),
                state.get("independent_axes"),
                state.get("diagonal_programs", {}).get(char),
            ],
            sort_keys=True,
        ).encode()
    )


def candidates(state, char, salt, structural_budget):
    # Wider jumps cross raster plateaus which +/-16, then +/-8, then +/-4 cannot
    # leave if every intermediate raster is unchanged and costs more instructions.
    for step in (64, 32, 16, 8):
        for axis in (0, 1):
            yield from proposals(state, char, axis, step, False)
    for axis in (0, 1):
        groups = state["graph"][char][axis]["groups"]
        for branch, nodes in enumerate(axis_programs(state, char, axis)):
            for i, group in enumerate(groups):
                if not group.get("cross_axis"):
                    continue
                choices = [None]
                choices += [dict(op="anchor", round=mode) for mode in hints.ROUNDS]
                choices += [
                    dict(op="adjust", round="none", shift=shift)
                    for shift in (-64, -32, -16, 16, 32, 64)
                ]
                for node in choices:
                    if node == nodes[i]:
                        continue
                    candidate = copy_hint_state(state, char)
                    list(axis_programs(candidate, char, axis))[branch][i] = node
                    try:
                        hints.ordered(
                            list(axis_programs(candidate, char, axis))[branch],
                            state["graph"][char][axis],
                        )
                    except ValueError:
                        continue
                    yield candidate
        if structural_budget:
            yield from neighborhood(
                state, char, axis, 16, True, structural_budget, salt
            )


def search(
    initial, baseline, oracle, rounds, beam_width, structural_budget, callback=None
):
    (char,) = initial["shapes"]
    current = oracle.stats(initial)
    constraints = Constraints(oracle.stats(baseline), current)
    relaxed = Relaxed(constraints)
    best, best_stats = initial, current
    best_score = target_score(current, constraints, best, baseline)
    frontier = [(initial, current, False)]
    visited = {identity(initial, char)}
    history = []
    for depth in range(rounds):
        pool = [(best, best_stats, False)]
        counts = Counter()
        for parent, _, ancestry_violated in frontier:
            for candidate in candidates(parent, char, f"{depth}:", structural_budget):
                key = identity(candidate, char)
                if key in visited:
                    continue
                visited.add(key)
                stats = oracle.stats(candidate)
                violations = constraints.measure(stats)[1]
                counts.update(violations)
                counts["evaluated"] += 1
                pool.append((candidate, stats, ancestry_violated or bool(violations)))
                score = target_score(stats, constraints, candidate, baseline)
                if score < best_score:
                    best, best_stats, best_score = candidate, stats, score
                    counts["accepted"] += 1
                    counts["accepted_after_regression"] += ancestry_violated
        # Separate feasible, target-first and feasibility-repair lanes. Do not
        # deduplicate by IoU/count vectors: equally wrong rasters can occupy
        # different pixels and behave differently under the next edit.
        lanes = [
            sorted(
                (r for r in pool if not constraints.measure(r[1])[1]),
                key=lambda r: target_score(r[1], constraints, r[0], baseline),
            ),
            sorted(
                pool,
                key=lambda r: (
                    target_score(r[1], relaxed, r[0], baseline)[:2],
                    constraints.excess(r[1]),
                    identity(r[0], char),
                ),
            ),
            sorted(
                pool,
                key=lambda r: (
                    constraints.excess(r[1]),
                    target_score(r[1], relaxed, r[0], baseline),
                    identity(r[0], char),
                ),
            ),
        ]
        frontier, chosen = [], set()
        for rank in range(beam_width):
            for lane in lanes:
                if rank >= len(lane):
                    continue
                row = lane[rank]
                key = identity(row[0], char)
                if key not in chosen:
                    frontier.append(row)
                    chosen.add(key)
                    if len(frontier) >= beam_width:
                        break
            if len(frontier) >= beam_width:
                break
        record = dict(
            depth=depth,
            counts=dict(counts),
            evaluated_programs=len(visited) - 1,
            score=best_score,
            **metrics(best_stats),
        )
        history.append(record)
        if callback:
            callback(best, record)
        if metrics(best_stats)["passes_target"] or not counts["evaluated"]:
            break
    require(not constraints.measure(best_stats)[1], "repair output violates a baseline")
    return best, best_stats, history


def diagonal_repair(initial, baseline, oracle, constraints, callback=None):
    """Test one geometry-derived stroke correction across coarse optical ranges."""
    (char,) = initial["shapes"]
    current = oracle.stats(initial)
    if not diagonal_font_hints.strokes(initial["shapes"][char]):
        return initial, current, []
    best, best_stats = initial, current
    best_score = target_score(current, constraints, initial, baseline)
    counts = Counter()
    for start in (10, 12, 16, 24):
        for limit in (24, 32, 48, 64, 90):
            if not start < limit <= initial["parameters"]["limit"]:
                continue
            for shift in (-32, -24, -16, -8, 8, 16, 24, 32):
                candidate = copy_hint_state(initial, char)
                candidate.setdefault("diagonal_programs", {})[char] = dict(
                    start=start, limit=limit, shift=shift
                )
                stats = oracle.stats(candidate)
                violations = constraints.measure(stats)[1]
                counts.update(violations)
                counts["evaluated"] += 1
                score = target_score(stats, constraints, candidate, baseline)
                if not violations and score < best_score:
                    best, best_stats, best_score = candidate, stats, score
                    counts["accepted"] += 1
    record = dict(stage="diagonal", counts=dict(counts), **metrics(best_stats))
    if callback:
        callback(best, record)
    return best, best_stats, [record]


def worker(task):
    char, start, baseline, references, engine, root, settings = task
    root = root / f"{ord(char):03}"
    root.mkdir(parents=True)
    cache = root / "cache"
    runner = pipeline.Engine(engine, cache, 100000)
    initial = complete_axes(start) if settings["complete_axes"] else start
    oracle = Oracle(
        runner,
        references,
        initial,
        projections=settings["strategy"] in ("projection", "combined"),
    )
    require(
        oracle.stats(initial) == oracle.stats(start),
        "feature expansion changed rasters",
    )

    def checkpoint(state, row):
        save(root / "checkpoint.json", state)
        print(json.dumps(dict(glyph=char, **row)), flush=True)

    state, stats, history = initial, oracle.stats(initial), []
    constraints = Constraints(oracle.stats(baseline), stats)
    if settings["strategy"] in ("projection", "combined"):
        state, stats, history = projection_hint_search.repair(
            state,
            baseline,
            oracle,
            constraints,
            rounds=settings["rounds"],
            callback=checkpoint,
        )
    if settings["strategy"] in ("beam", "combined"):
        state, stats, subsequent = search(
            state,
            baseline,
            oracle,
            settings["rounds"],
            settings["beam"],
            settings["structural_budget"],
            checkpoint,
        )
        history.extend(subsequent)
    if settings["strategy"] == "diagonal":
        state, stats, history = diagonal_repair(
            state, baseline, oracle, constraints, checkpoint
        )
    require(
        ascii_fit.score(
            runner.render(hints.build(state), sorted(references)), references
        )
        == stats,
        "repair cache audit failed",
    )
    save(root / "state.json", state)
    save(
        root / "report.json",
        dict(
            glyph=char,
            initial=metrics(oracle.stats(start)),
            final=metrics(stats),
            stages=history,
            stats=[dict(query=q, **s) for q, s in sorted(stats.items())],
        ),
    )
    return char, state, stats


def development(fixtures):
    pages, provenance, _ = ascii_fit.load(
        fixtures / "ascii-font-20261004", {"outline", "hints"}
    )
    six = json.loads(
        (fixtures / "target-hints-20261004/development-provenance.json").read_text()
    )
    for artifact in six["artifacts"]:
        require(
            sha((fixtures / artifact["path"]).read_bytes()) == artifact["sha256"],
            "six-glyph development artifact changed",
        )
    for path in six["capture_paths"]:
        selected, source = pipeline.load_pages(fixtures / path, "development")
        pages.extend(selected)
        provenance.append(dict(path=path, **source))
    return pages, provenance


def fit(args):
    require(
        1 <= args.workers <= 8
        and 1 <= args.glyphs <= 94
        and 1 <= args.rounds <= 12
        and 1 <= args.beam <= 12
        and 0 <= args.structural_budget <= 2048,
        "repair search budget exceeded",
    )
    data = (args.seed / "candidates.json").read_bytes()
    seed_report = json.loads((args.seed / "report.json").read_text())
    require(sha(data) == seed_report["candidates_sha256"], "seed candidates changed")
    states = json.loads(data)
    initial = states["targeted"]
    baseline = states["baseline"]
    plan_data = (args.seed / "plan.json").read_bytes()
    require(sha(plan_data) == seed_report["plan_sha256"], "seed plan changed")
    prior = json.loads(plan_data)
    for label in ("baseline", "targeted"):
        require(
            sha(hints.build(states[label]))
            == seed_report["variants"][label]["font_sha256"],
            "seed compiler output changed",
        )
    engine_hash = sha(args.engine.read_bytes())
    changed_engine = engine_hash != prior["engine_sha256"]
    if changed_engine:
        require(
            args.previous_engine is not None
            and sha(args.previous_engine.read_bytes()) == prior["engine_sha256"],
            "engine migration needs the verified previous executable",
        )
    pages, provenance = development(args.fixtures)
    references = pipeline.cases(pages)
    train, check = (
        set(map(tuple, prior[k])) for k in ("search_queries", "internal_check_queries")
    )
    require(
        not train & check and train | check == references.keys(),
        "development split changed",
    )
    args.output.mkdir(parents=True, exist_ok=False)
    runner = pipeline.Engine(args.engine, args.output / "audit-cache", 8)
    if changed_engine:
        previous = pipeline.Engine(
            args.previous_engine, args.output / "previous-cache", 8
        )
        for state in (baseline, initial):
            font = hints.build(state)

            def raster_rows(engine):
                return {
                    q: {k: v for k, v in row.items() if k != "contours"}
                    for q, row in engine.render(font, sorted(references)).items()
                }

            require(
                raster_rows(previous) == raster_rows(runner),
                "engine migration changes seed rasters or metrics",
            )
    before = ascii_fit.score(
        runner.render(hints.build(initial), sorted(train)),
        {q: references[q] for q in train},
    )
    order = sorted(
        initial["shapes"],
        key=lambda c: (-max(error(before[q]) for q in train if q[2] == ord(c)), c),
    )
    selected = order[: args.glyphs]
    settings = dict(
        schema="font-repair-search-v1",
        seed_candidates_sha256=sha(data),
        engine_sha256=engine_hash,
        engine_migration=(
            dict(
                previous_sha256=prior["engine_sha256"],
                seed_fonts=2,
                exact_queries_per_font=len(references),
            )
            if changed_engine
            else None
        ),
        search_queries=sorted(train),
        internal_check_queries=sorted(check),
        captures=provenance,
        glyph_order=selected,
        complete_axes=not args.no_complete_axes,
        rounds=args.rounds,
        beam=args.beam,
        structural_budget=args.structural_budget,
        strategy=args.strategy,
        selection="training only; both historical and incumbent guards",
        printer=prior["printer"],
    )
    source = args.output / "source"
    source.mkdir()
    for name in (
        "repair_font_search",
        "fit_ascii_hints",
        "fit_font_target",
        "fit_structured_hints",
        "joint_hint_program",
        "structured_font_hints",
        "reconstruct_font",
        "font_probe",
        "projection_hint_search",
        "diagonal_font_hints",
    ):
        path = Path(__file__).with_name(name + ".py")
        (source / path.name).write_bytes(path.read_bytes())
    settings["scripts"] = {p.name: sha(p.read_bytes()) for p in source.iterdir()}
    save(args.output / "plan.json", settings)
    state, scores = deepcopy(initial), {}
    tasks = [
        (
            c,
            one_glyph(initial, c),
            one_glyph(baseline, c),
            {q: references[q] for q in train if q[2] == ord(c)},
            args.engine,
            args.output / "glyphs",
            settings,
        )
        for c in selected
    ]
    with ProcessPoolExecutor(max_workers=args.workers) as pool:
        for future in as_completed([pool.submit(worker, task) for task in tasks]):
            char, fitted, stats = future.result()
            replace_glyph(state, fitted, char)
            scores.update(stats)
    save(args.output / "candidates.json", dict(baseline=initial, targeted=state))
    for label, variant in (("baseline", initial), ("targeted", state)):
        (args.output / (label + ".ttf")).write_bytes(hints.build(variant))
    frozen_at = now()
    report = dict(
        schema="font-repair-search-v1",
        frozen_at=frozen_at,
        plan_sha256=sha((args.output / "plan.json").read_bytes()),
        candidates_sha256=sha((args.output / "candidates.json").read_bytes()),
        designated="targeted",
        production_ready=False,
        variants={},
    )
    for label, variant in (("baseline", initial), ("targeted", state)):
        rows = runner.render(hints.build(variant), sorted(references))
        stats = ascii_fit.score(rows, references)
        if label == "targeted":
            require(
                all(stats[q] == s for q, s in scores.items()),
                "whole-font repair audit failed",
            )
            require(
                not Constraints({q: before[q] for q in train}, before).measure(
                    {q: stats[q] for q in train}
                )[1],
                "assembled font regressed",
            )
        scored = pipeline.engine_api.score(pages, rows)
        examination.report_cases(scored)
        report["variants"][label] = dict(
            font_sha256=sha(hints.build(variant)),
            cache_audit_exact=True,
            search_target=metrics({q: stats[q] for q in train}),
            check_target=metrics({q: stats[q] for q in check}),
            pages=scored,
        )
    save(args.output / "report.json", report)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("fixtures", "seed", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--previous-engine", type=Path)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--glyphs", type=int, default=8)
    parser.add_argument("--rounds", type=int, default=4)
    parser.add_argument("--beam", type=int, default=4)
    parser.add_argument("--structural-budget", type=int, default=128)
    parser.add_argument("--no-complete-axes", action="store_true")
    parser.add_argument(
        "--strategy",
        choices=("beam", "projection", "combined", "diagonal"),
        default="beam",
    )
    fit(parser.parse_args())
