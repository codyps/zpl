"""Diagnose a frozen joint reconstruction and run bounded development ablations.

All comparisons preserve the captured origin. Search uses only the frozen
model's search_queries. The old internal check is an audit, not a fresh holdout;
it is scored only after every candidate and the training-selected winner freeze.
This experiment never selects a production font or changes previous artifacts.
"""

import argparse
from copy import deepcopy
import json
from pathlib import Path

from compare_font_fits import stratum, summarize
import joint_font_geometry as geometry
import joint_hint_program as hints
import optimize_font as optimizer
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha


def breakdown(cases):
    require(bool(cases), "empty diagnostic cases")
    return dict(
        total=summarize(cases),
        sizes={
            band: summarize([c for c in cases if stratum(c) == band])
            for band in sorted({stratum(c) for c in cases})
        },
        glyphs={
            char: summarize([c for c in cases if c["text"] == char])
            for char in sorted({c["text"] for c in cases})
        },
        glyph_sizes={
            char: {
                band: summarize(
                    [c for c in cases if c["text"] == char and stratum(c) == band]
                )
                for band in sorted({stratum(c) for c in cases if c["text"] == char})
            }
            for char in sorted({c["text"] for c in cases})
        },
        transforms={
            label: summarize(selected)
            for label in ("square", "stretched", "rotated")
            if (
                selected := [
                    c
                    for c in cases
                    if (
                        "rotated"
                        if c["orientation"] != "N"
                        else (
                            "square"
                            if (c["width"] or c["height"]) == c["height"]
                            else "stretched"
                        )
                    )
                    == label
                ]
            )
        },
        worst=sorted(
            cases,
            key=lambda c: (-(c["xor"] / c["union"] if c["union"] else 0), c["text"]),
        )[:18],
    )


def report_cases(report):
    """Require that strata account for every pixel in every native canvas."""
    result = []
    for page in report:
        for key in ("under", "over", "xor", "union"):
            require(
                sum(c[key] for c in page["cases"]) == page[key],
                "case counts do not cover native canvas",
            )
        result.extend(page["cases"])
    return result


def development(model_root, captures):
    """Match complete provenance, then use the model's already-frozen split."""
    model, digest = pipeline.frozen(model_root)
    seal = json.loads((model_root / "inputs.json").read_text())
    expected = [seal["source"], seal["development"]]
    expected += seal["extra_source"] + seal["extra_development"]
    references, observed, pages = {}, [], []
    for path in captures:
        selected, provenance = pipeline.load_pages(path, "development")
        require(provenance in expected, "capture is not a frozen development input")
        require(provenance not in observed, "duplicate development campaign")
        observed.append(provenance)
        cases = pipeline.cases(selected)
        require(not (references.keys() & cases.keys()), "duplicate development case")
        references.update(cases)
        pages.extend(selected)
    require(len(observed) == len(expected), "missing frozen development input")
    train, check = (
        set(map(tuple, model[k])) for k in ("search_queries", "internal_check_queries")
    )
    require(not (train & check), "search overlaps internal check")
    require(
        train | check == references.keys(), "development split does not cover inputs"
    )
    return model, digest, seal, references, pages, train, check


def without_axes(state, axes):
    result = deepcopy(state)
    for program in result["programs"].values():
        for axis in axes:
            program[axis] = [None] * len(program[axis])
    return result


def run(args):
    model, digest, inputs, references, pages, train, check = development(
        args.model, args.captures
    )
    seed, _ = pipeline.frozen(args.seed)
    require(
        sha((args.seed / "model.json").read_bytes()) == inputs["seed_sha256"],
        "seed differs",
    )
    baseline, original = model["state"], hints.initialize(seed)
    engine = pipeline.Engine(args.engine, args.cache, 20000)
    require(engine.digest == inputs["engine_sha256"], "diagnostic engine differs")
    args.output.mkdir(parents=True, exist_ok=False)
    settings = dict(
        model_sha256=digest,
        engine_sha256=engine.digest,
        scripts={
            name: sha(Path(__file__).with_name(name).read_bytes())
            for name in (
                Path(__file__).name,
                "optimize_font.py",
                "joint_hint_program.py",
                "joint_font_geometry.py",
                "reconstruct_font.py",
                "compare_font_fits.py",
            )
        },
        hint_sweeps=2,
        hint_neighborhood="all distinct valid one-edit neighbors",
        geometry_steps=[4, 2, 1, 1, 1, 1],
        geometry_bound_from_original_seed=12,
        geometry_proposals=24,
        fixed_ablations=[
            "no_hints",
            "x_only",
            "y_only",
            "seed_geometry",
            "seed_hints",
            "unlimited_hints",
        ],
        selection="lowest original objective under individual large-query constraints; search_queries only",
        audit="previously used internal check; no fresh validation or production promotion",
        search_queries=sorted(train),
        internal_check_queries=sorted(check),
    )
    save(args.output / "plan.json", settings)
    oracle = optimizer.Oracle(
        engine, {q: references[q] for q in sorted(train)}, original
    )
    candidates = {"baseline": deepcopy(baseline)}
    for label, axes in (("no_hints", (0, 1)), ("x_only", (1,)), ("y_only", (0,))):
        candidates[label] = without_axes(baseline, axes)
    candidates["seed_geometry"] = deepcopy(baseline)
    candidates["seed_geometry"]["shapes"] = deepcopy(original["shapes"])
    candidates["seed_hints"] = deepcopy(baseline)
    candidates["seed_hints"]["programs"] = deepcopy(original["programs"])
    candidates["seed_hints"]["parameters"] = deepcopy(original["parameters"])
    candidates["unlimited_hints"] = deepcopy(baseline)
    candidates["unlimited_hints"]["parameters"]["limit"] = 4096
    progress = []

    def record(label, state, **details):
        errors = oracle.errors(state)
        item = dict(
            stage=label,
            group_errors=optimizer.means(errors),
            objective=optimizer.objective(state, original, errors),
            large_regressions=sum(
                e > oracle.floor[q] + 1e-12
                for q, e in errors.items()
                if optimizer.category(q) == "large"
            ),
            evaluations=len(engine.seen),
            **details,
        )
        progress.append(item)
        save(args.output / "progress.json", progress)
        print(json.dumps(item), flush=True)

    for label, state in candidates.items():
        record(label, state)

    state = deepcopy(baseline)
    for sweep in range(settings["hint_sweeps"]):
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
            record(
                "hint_refinement",
                state,
                sweep=sweep,
                glyph=char,
                neighbors=len(seen) - 1,
            )
    candidates["hint_refinement"] = state

    state = deepcopy(baseline)
    large = {
        q: references[q] for q in sorted(train) if optimizer.category(q) == "large"
    }
    guides = {c: geometry.Guide(large, ord(c)) for c in state["shapes"]}
    for sweep, step in enumerate(settings["geometry_steps"]):
        for char in sorted(state["shapes"]):
            best, score, move = state, oracle.score(state), None
            for axis, indices, delta in guides[char].proposals(
                state["shapes"][char],
                state["graph"][char],
                step,
                settings["geometry_proposals"],
            ):
                shape = geometry.moved(
                    state["shapes"][char],
                    original["shapes"][char],
                    axis,
                    indices,
                    delta,
                )
                if shape is None:
                    continue
                candidate = deepcopy(state)
                candidate["shapes"][char] = shape
                measured = oracle.score(candidate)
                if measured < score - 1e-12:
                    best, score, move = candidate, measured, [axis, indices, delta]
            state = best
            record("geometry_refinement", state, sweep=sweep, glyph=char, move=move)
    candidates["geometry_refinement"] = state
    combined = deepcopy(state)
    combined["programs"] = deepcopy(candidates["hint_refinement"]["programs"])
    candidates["combined"] = combined
    record("combined", combined)
    winner = min(candidates, key=lambda label: (oracle.score(candidates[label]), label))
    # Freeze bytes and selection before scoring any previous check pixels.
    save(args.output / "candidates.json", dict(winner=winner, states=candidates))
    for label, candidate in candidates.items():
        (args.output / (label + ".ttf")).write_bytes(hints.build(candidate))

    report = dict(
        schema="font-accuracy-examination-v1",
        plan_sha256=sha((args.output / "plan.json").read_bytes()),
        candidates_sha256=sha((args.output / "candidates.json").read_bytes()),
        winner=winner,
        production_ready=False,
        variants={},
    )
    for label, candidate in candidates.items():
        data = (args.output / (label + ".ttf")).read_bytes()
        rows = engine.render(data, sorted(references))
        page_scores = pipeline.engine_api.score(pages, rows)
        cases = report_cases(page_scores)
        keyed = {
            (
                c["width"] or c["height"],
                c["height"],
                ord(c["text"]),
                "NRIB".index(c["orientation"]),
            ): c
            for c in cases
        }
        errors = oracle.errors(candidate)
        report["variants"][label] = dict(
            font_sha256=sha(data),
            training_objective=optimizer.objective(candidate, original, errors),
            large_regressions=sum(
                e > oracle.floor[q] + 1e-12
                for q, e in errors.items()
                if optimizer.category(q) == "large"
            ),
            search=breakdown([keyed[q] for q in sorted(train)]),
            previous_internal_check=breakdown([keyed[q] for q in sorted(check)]),
            pages=page_scores,
        )
        save(args.output / "report.json", report)
        print(
            json.dumps(
                dict(
                    evaluated=label,
                    search=report["variants"][label]["search"]["total"],
                    check=report["variants"][label]["previous_internal_check"]["total"],
                )
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("seed", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--captures", nargs="+", type=Path, required=True)
    run(parser.parse_args())
