"""Fit ASCII glyph hint programs independently under per-glyph hard constraints.

Each worker sees only its glyph's training pixels. CVTs and outlines stay fixed,
so workers cannot affect each other. Merging is audited by executing the complete
TTF and comparing every count with the individual fits. Native internal checks
are read for reporting only after font/state artifacts freeze.
"""

import argparse
from concurrent.futures import ProcessPoolExecutor, as_completed
from copy import deepcopy
import json
from pathlib import Path

import examine_font_accuracy as examination
from fit_font_target import metrics, proposals, screen_reject, target_score
from fit_structured_hints import Objective, Oracle, error
import joint_hint_program as hints
import reconstruct_ascii_font as bootstrap
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
import split_hint_features
import structured_font_hints as structure

GLYPH_MAPS = ("shapes", "graph", "programs", "regimes", "optical_programs")


def one_glyph(state, char):
    result = deepcopy(state)
    for key in GLYPH_MAPS:
        if key in result:
            result[key] = {char: result[key][char]} if char in result[key] else {}
    if "independent_axes" in result:
        result["independent_axes"] = [
            c for c in result["independent_axes"] if c == char
        ]
    return result


def replace_glyph(state, fitted, char):
    for key in GLYPH_MAPS:
        if key in fitted:
            if char in fitted[key]:
                state.setdefault(key, {})[char] = deepcopy(fitted[key][char])
            elif key in state:
                state[key].pop(char, None)
    if char in fitted.get("independent_axes", []):
        state["independent_axes"] = sorted(
            set(state.get("independent_axes", [])) | {char}
        )
    elif "independent_axes" in state:
        state["independent_axes"] = [c for c in state["independent_axes"] if c != char]


def neighborhood(state, char, axis, step, structural, limit, salt):
    # Keep a reproducible hash sample of the entire valid neighborhood, rather
    # than truncating early and starving later branches/features. Generate at
    # most one copy of each program/range/guard combination.
    selected = {}
    for candidate in proposals(state, char, axis, step, structural):
        identity = json.dumps(
            [
                candidate["programs"][char],
                candidate.get("regimes"),
                candidate.get("optical_programs"),
                candidate.get("independent_axes"),
            ],
            sort_keys=True,
        )
        key = sha((salt + identity).encode())
        selected[key] = candidate
        if len(selected) > limit * 2:
            selected = {k: selected[k] for k in sorted(selected)[:limit]}
    return [selected[k] for k in sorted(selected)[:limit]]


def worker(task):
    char, baseline, warm, references, engine, root, cache_root, settings = task
    output = root / f"{ord(char):03}"
    output.mkdir(parents=True, exist_ok=False)
    save(output / "baseline.json", baseline)
    cache = cache_root / f"{ord(char):03}" / "cache"
    cache.mkdir(parents=True, exist_ok=True)
    engine = pipeline.Engine(engine, cache, 100000)
    save(output / "warm-start.json", warm)
    initial = split_hint_features.split(
        structure.curve_shoulders(structure.curved_stems(warm))
    )
    oracle = Oracle(engine, references, initial)
    floor = oracle.stats(baseline)
    require(
        oracle.stats(initial) == oracle.stats(warm),
        "initial graph expansion changed raster counts",
    )
    objective = Objective(floor, "robust")
    state, current = initial, oracle.stats(initial)
    require(
        target_score(current, objective, state, baseline)[0] != float("inf"),
        "warm start violates original constraints",
    )
    critical = sorted(
        references, key=lambda q: (floor[q]["xor"] != 0, -error(floor[q]), q)
    )[:12]
    stages = []
    for sweep, step in enumerate(settings["steps"]):
        if metrics(current)["passes_target"]:
            break
        for axis in (0, 1):
            for local in range(settings["local_steps"]):
                before = target_score(current, objective, state, baseline)
                best, best_stats, best_score = state, current, before
                selected = neighborhood(
                    state,
                    char,
                    axis,
                    step,
                    True,
                    settings["neighbors"],
                    f"{sweep}:{axis}:{local}:",
                )
                for candidate in selected:
                    if screen_reject(oracle.stats(candidate, critical), floor):
                        continue
                    stats = oracle.stats(candidate)
                    score = target_score(stats, objective, candidate, baseline)
                    if score < best_score:
                        best, best_stats, best_score = candidate, stats, score
                row = dict(
                    sweep=sweep,
                    axis=axis,
                    local=local,
                    before=before,
                    after=best_score,
                    proposals=len(selected),
                    **metrics(best_stats),
                )
                stages.append(row)
                state, current = best, best_stats
                save(output / "checkpoint.json", state)
                save(output / "progress.json", stages)
                if best_score >= before or metrics(current)["passes_target"]:
                    break
    data = hints.build(state)
    audited = bootstrap.score(engine.render(data, sorted(references)), references)
    require(audited == current, "individual-glyph cache audit failed")
    save(output / "state.json", state)
    save(
        output / "result.json",
        dict(
            glyph=char,
            baseline=metrics(floor),
            targeted=metrics(current),
            stats=[dict(query=q, **s) for q, s in sorted(current.items())],
            state_sha256=sha((output / "state.json").read_bytes()),
        ),
    )
    return char, state, current


def fit(
    sampling,
    seed,
    output,
    engine,
    workers=4,
    neighbors=512,
    steps=(16, 8, 4),
    cache_root=None,
    warm_start=None,
):
    data = (seed / "state.json").read_bytes()
    baseline = json.loads(data)
    report = json.loads((seed / "report.json").read_text())
    require(
        report["state_sha256"] == sha(data)
        and report["font_sha256"] == sha(hints.build(baseline)),
        "bootstrap seed changed",
    )
    pages, provenance, digest = bootstrap.load(sampling, {"outline", "hints"})
    prior = json.loads((seed / "plan.json").read_text())
    require(
        prior["sampling_sha256"] == digest and prior["captures"] == provenance,
        "development inputs changed",
    )
    references = pipeline.cases(pages)
    train = set(map(tuple, prior["search_queries"]))
    check = set(map(tuple, prior["internal_check_queries"]))
    require(
        not train & check and train | check == set(references), "invalid training split"
    )
    output.mkdir(parents=True, exist_ok=False)
    cache_root = cache_root or output / "glyphs"
    warm = structure.initialize(baseline)
    warm_inputs = {}
    if warm_start:
        for char in baseline["shapes"]:
            path = warm_start / "glyphs" / f"{ord(char):03}" / "checkpoint.json"
            if path.exists():
                warm_data = path.read_bytes()
                candidate = json.loads(warm_data)
                require(
                    candidate["shapes"][char] == baseline["shapes"][char]
                    and candidate["parameters"] == baseline["parameters"],
                    "warm start changed fixed geometry or CVTs",
                )
                replace_glyph(warm, candidate, char)
                warm_inputs[char] = dict(path=str(path), sha256=sha(warm_data))
    save(output / "warm-start.json", warm)
    require(
        1 <= workers <= 8 and 32 <= neighbors <= 8192,
        "worker/neighborhood budget exceeded",
    )
    settings = dict(
        schema="ascii-hint-fit-v1",
        steps=list(steps),
        neighbors=neighbors,
        local_steps=2,
        selection="per-glyph minimax with robust baseline guards; training only",
        seed_sha256=sha(data),
        sampling_sha256=digest,
        engine_sha256=sha(engine.read_bytes()),
        workers=workers,
        search_queries=sorted(train),
        internal_check_queries=sorted(check),
        warm_inputs=warm_inputs,
        warm_start_sha256=sha((output / "warm-start.json").read_bytes()),
    )
    require(
        settings["engine_sha256"] == prior["engine_sha256"], "fitting engine changed"
    )
    source = output / "source"
    source.mkdir()
    for name in (
        "fit_ascii_hints",
        "fit_font_target",
        "fit_structured_hints",
        "structured_font_hints",
        "split_hint_features",
        "joint_hint_program",
        "font_probe",
        "reconstruct_ascii_font",
        "reconstruct_font",
        "reconstruct_geometry",
        "refine_font_features",
    ):
        path = Path(__file__).with_name(name + ".py")
        (source / path.name).write_bytes(path.read_bytes())
    settings["scripts"] = {p.name: sha(p.read_bytes()) for p in source.iterdir()}
    save(output / "plan.json", settings)
    save(output / "baseline.json", baseline)
    tasks = [
        (
            c,
            one_glyph(baseline, c),
            one_glyph(warm, c),
            {q: references[q] for q in train if q[2] == ord(c)},
            engine,
            output / "glyphs",
            cache_root,
            settings,
        )
        for c in sorted(baseline["shapes"])
    ]
    state = deepcopy(baseline)
    scores = {}
    with ProcessPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(worker, task) for task in tasks]
        for future in as_completed(futures):
            char, fitted, stats = future.result()
            replace_glyph(state, fitted, char)
            scores.update(stats)
            save(output / "checkpoint.json", state)
            print(json.dumps(dict(glyph=char, **metrics(stats))), flush=True)
    save(output / "state.json", state)
    (output / "font.ttf").write_bytes(hints.build(state))
    runner = pipeline.Engine(engine, output / "audit-cache", 4)
    rows = runner.render(hints.build(state), sorted(references))
    stats = bootstrap.score(rows, references)
    require(
        all(stats[q] == s for q, s in scores.items()),
        "whole-font merge cache audit failed",
    )
    scored = pipeline.engine_api.score(pages, rows)
    examination.report_cases(scored)
    save(
        output / "report.json",
        dict(
            schema=settings["schema"],
            plan_sha256=sha((output / "plan.json").read_bytes()),
            state_sha256=sha((output / "state.json").read_bytes()),
            font_sha256=sha((output / "font.ttf").read_bytes()),
            cache_audit_exact=True,
            search=metrics({q: stats[q] for q in train}),
            internal_check=metrics({q: stats[q] for q in check}),
            pages=scored,
            production_ready=False,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sampling", "seed", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--neighbors", type=int, default=512)
    parser.add_argument("--steps", type=int, nargs="+", default=[16, 8, 4])
    parser.add_argument("--cache-root", type=Path)
    parser.add_argument("--warm-start", type=Path)
    args = parser.parse_args()
    fit(
        args.sampling,
        args.seed,
        args.output,
        args.engine,
        args.workers,
        args.neighbors,
        args.steps,
        args.cache_root,
        args.warm_start,
    )
