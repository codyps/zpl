"""Bootstrap new ASCII outlines and hints from a predeclared capture campaign.

The six research glyphs keep their independently frozen model and measurements.
This stage reconstructs the other 88 visible glyphs and all 95 hmtx advances.
Validation PNGs are never opened. The result is an experimental reusable TTF,
not a bitmap strike collection or a production Font 0 replacement.
"""

import argparse
from copy import deepcopy
from concurrent.futures import ProcessPoolExecutor, as_completed
import itertools
import json
from pathlib import Path

from ascii_font_probe import NEW_GLYPHS, PRINTABLE
import ascii_font_metrics as spacing
from fit_font_target import metrics
import font0_hint_model as api
import joint_font_geometry as geometry_search
import joint_hint_program as hints
import optimize_font
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
import reconstruct_geometry as geometry
import reconstruct_hints


def load(sampling, roles):
    data = (sampling / "plan.json").read_bytes()
    plan = json.loads(data)
    require(plan["schema"] == "ascii-font-sampling-v1", "unknown ASCII plan")
    pages, provenance = [], []
    for campaign in plan["campaigns"]:
        if campaign["group"] != "development" or campaign["role"] not in roles:
            continue
        root = sampling / campaign["name"]
        require(
            sha((root / "manifest.json").read_bytes()) == campaign["manifest_sha256"],
            "sampling manifest changed",
        )
        selected, source = pipeline.load_pages(
            root, "development", isolated=campaign["role"] != "spacing"
        )
        pages.extend(selected)
        provenance.append(dict(path=campaign["name"], role=campaign["role"], **source))
    require(pages, "missing development captures")
    require(
        all(p["printer"] == provenance[0]["printer"] for p in provenance),
        "capture environments differ",
    )
    return pages, provenance, sha(data)


def score(rows, references):
    return {
        q: pipeline.counts(reference, pipeline.pixels(rows[q]))
        for q, reference in references.items()
    }


def rank(stats):
    values = metrics(stats)
    deficits = [max(0, s["xor"] / max(1, s["union"]) - 0.1) for s in stats.values()]
    return (
        round(max(deficits), 12),
        round(sum(d * d for d in deficits) / len(deficits), 12),
        round(1 - values["mean_iou"], 12),
        round(1 - values["pooled_iou"], 12),
    )


def recover_glyph(task):
    char, key, points, cases, settings, engine, cache = task
    runner = pipeline.Engine(engine, cache, 1000)
    # Legacy scaling and calibrated pixel-cell scaling both remain candidates.
    choices = []
    loops = api.contours(points)
    for label, scale in (
        ("legacy", 2048 / key[1]),
        ("calibrated", 2048 / pipeline.ppem(key[1])),
    ):
        choices.append(
            (
                label,
                [
                    [(round(x * scale), round(-y * scale), True) for x, y in c]
                    for c in loops
                ],
            )
        )
    rejected = []
    for label, scale in (
        ("legacy", 2048 / key[1]),
        ("calibrated", 2048 / pipeline.ppem(key[1])),
    ):
        for tolerance in settings["tolerances"]:
            try:
                choices.append(
                    (
                        f"quadratic-{label}-{tolerance}",
                        geometry.fit_shape(points, tolerance, scale),
                    )
                )
            except geometry.FitRejected as error:
                rejected.append(
                    dict(scale=label, tolerance=tolerance, error=str(error))
                )
    ranked = []
    for label, shape in choices:
        stats = score(runner.render(api.build_font({char: shape}), cases), cases)
        points_count = sum(map(len, shape))
        empirical = rank(stats)
        geometric = (
            *empirical[:2],
            empirical[2] + settings["point_penalty"] * points_count,
        )
        ranked.append((geometric, points_count, label, shape, stats))
    _, _, label, shape, stats = min(ranked, key=lambda v: v[:3])
    original = deepcopy(shape)
    graph = [reconstruct_hints.features(shape, a) for a in (0, 1)]
    guide = geometry_search.Guide(cases, ord(char))
    moves = []
    for step in settings["geometry_steps"]:
        best, best_stats, move = shape, stats, None
        for axis, indices, delta in guide.proposals(
            shape, graph, step, settings["geometry_proposals"]
        ):
            candidate = geometry_search.moved(shape, original, axis, indices, delta)
            if candidate is None:
                continue
            measured = score(
                runner.render(api.build_font({char: candidate}), cases), cases
            )
            # Preserve each large case, not only pooled silhouette accuracy.
            if any(
                measured[q]["xor"] / max(1, measured[q]["union"])
                > stats[q]["xor"] / max(1, stats[q]["union"]) + 1e-12
                for q in cases
            ):
                continue
            if rank(measured) < rank(best_stats):
                best, best_stats, move = candidate, measured, [axis, indices, delta]
        shape, stats = best, best_stats
        moves.append(dict(step=step, move=move, metrics=metrics(stats)))
    decision = dict(
        selected=label,
        rejected=rejected,
        moves=moves,
        candidates=[
            dict(label=v[2], points=v[1], metrics=metrics(v[4])) for v in ranked
        ],
        final=metrics(stats),
    )
    return char, shape, decision


def recover(sampling, output, engine, cache, workers=1, resume=False):
    pages, provenance, digest = load(sampling, {"outline"})
    sources = pipeline.source_shapes(pages)
    require(set(sources) == set(NEW_GLYPHS), "incomplete new ASCII outline coverage")
    references = pipeline.cases(pages)
    require(1 <= workers <= 8, "worker budget exceeded")
    output.mkdir(parents=True, exist_ok=resume)
    runner = pipeline.Engine(engine, cache, 20000)
    settings = dict(
        schema="ascii-outline-reconstruction-v1",
        sampling_sha256=digest,
        captures=provenance,
        engine_sha256=runner.digest,
        tolerances=[0.4, 0.65, 0.9, 1.25, 1.75],
        geometry_steps=[8, 4, 2, 1],
        geometry_proposals=24,
        shape_budget_from_initializer=12,
        point_penalty=1e-5,
        scale_candidates=["legacy", "calibrated"],
    )
    script_names = (
        "reconstruct_ascii_font",
        "reconstruct_geometry",
        "reconstruct_hints",
        "joint_font_geometry",
        "joint_hint_program",
        "font0_hint_model",
        "font_probe",
        "reconstruct_font",
        "ascii_font_metrics",
        "ascii_font_probe",
        "fit_font_target",
    )
    settings["scripts"] = {
        name + ".py": sha(Path(__file__).with_name(name + ".py").read_bytes())
        for name in script_names
    }
    if resume:
        require(
            not (output / "outlines.json").exists(), "outline stage already complete"
        )
        require(
            json.loads((output / "plan.json").read_text()) == settings,
            "resume inputs changed",
        )
        checkpoint = json.loads((output / "checkpoint.json").read_text())
        shapes, decisions = checkpoint["shapes"], checkpoint["decisions"]
        require(
            set(shapes) == set(decisions) and set(shapes) <= set(sources),
            "invalid checkpoint",
        )
    else:
        save(output / "plan.json", settings)
        source = output / "source"
        source.mkdir()
        for name in settings["scripts"]:
            (source / name).write_bytes(Path(__file__).with_name(name).read_bytes())
        shapes, decisions = {}, {}
    pending = [
        (
            char,
            key,
            points,
            {q: p for q, p in references.items() if q[2] == ord(char)},
            settings,
            engine,
            cache,
        )
        for char, (key, points) in sources.items()
        if char not in shapes
    ]
    with ProcessPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(recover_glyph, task) for task in pending]
        failures = []
        for future in as_completed(futures):
            try:
                char, shape, decision = future.result()
            except Exception as error:
                failures.append(error)
                continue
            shapes[char], decisions[char] = shape, decision
            save(output / "checkpoint.json", dict(shapes=shapes, decisions=decisions))
            print(
                json.dumps(dict(glyph=char, stage="outline", **decision["final"])),
                flush=True,
            )
        if failures:
            raise failures[0]
    data = api.build_font(shapes)
    (output / "geometry.ttf").write_bytes(data)
    save(
        output / "outlines.json",
        dict(
            shapes=shapes,
            decisions=decisions,
            plan_sha256=sha((output / "plan.json").read_bytes()),
            font_sha256=sha(data),
        ),
    )


def fit(sampling, outlines, output, engine, cache):
    pages, provenance, digest = load(sampling, {"hints", "outline"})
    metrics_pages, metrics_provenance, _ = load(sampling, {"spacing"})
    outline_data = (outlines / "outlines.json").read_bytes()
    record = json.loads(outline_data)
    outline_plan = json.loads((outlines / "plan.json").read_text())
    require(
        record["plan_sha256"] == sha((outlines / "plan.json").read_bytes())
        and outline_plan["sampling_sha256"] == digest,
        "outline inputs changed",
    )
    shapes = record["shapes"]
    require(
        sha(api.build_font(shapes))
        == record["font_sha256"]
        == sha((outlines / "geometry.ttf").read_bytes()),
        "outline artifact changed",
    )
    all_cases = pipeline.cases(pages)
    large = {q: p for q, p in all_cases.items() if min(q[:2]) >= 128}
    small = {q: p for q, p in all_cases.items() if min(q[:2]) < 128}
    training, check = optimize_font.partition(large, small)
    advances, spacing_report = spacing.fit(spacing.extract(metrics_pages))
    require(set(advances) == set(PRINTABLE), "incomplete ASCII advances")
    inferred = reconstruct_hints.infer(shapes)
    inferred["hint_ppem_limit"] = 90
    state = hints.initialize(
        dict(
            shapes=shapes,
            inferred=inferred,
            cutin=24,
            policies={c: ["none", "none"] for c in shapes},
        )
    )
    state["advances"] = advances
    output.mkdir(parents=True, exist_ok=False)
    runner = pipeline.Engine(engine, cache, 10000)
    require(runner.digest == outline_plan["engine_sha256"], "outline engine changed")
    save(
        output / "plan.json",
        dict(
            schema="ascii-font-bootstrap-v1",
            sampling_sha256=digest,
            outline_sha256=sha(outline_data),
            captures=provenance,
            spacing_captures=metrics_provenance,
            engine_sha256=runner.digest,
            search_queries=sorted(training),
            internal_check_queries=sorted(check),
            selection="search only; freeze before internal-check evaluation",
        ),
    )
    policies = {}
    for char in sorted(shapes):
        references = {q: p for q, p in training.items() if q[2] == ord(char)}
        best = None
        for policy in itertools.product(reconstruct_hints.POLICIES, repeat=2):
            model = dict(
                shapes={char: shapes[char]},
                inferred=dict(inferred, graph={char: inferred["graph"][char]}),
                cutin=24,
                policies={char: policy},
            )
            candidate = hints.initialize(model)
            candidate["advances"] = advances
            stats = score(runner.render(hints.build(candidate), references), references)
            choice = (rank(stats), sum(p != "none" for p in policy), policy)
            if best is None or choice < best[0]:
                best = (choice, candidate["programs"][char], stats)
        state["programs"][char] = best[1]
        policies[char] = dict(policy=best[0][2], search=metrics(best[2]))
        save(output / "checkpoint.json", state)
        print(
            json.dumps(dict(glyph=char, stage="hint-bootstrap", **metrics(best[2]))),
            flush=True,
        )
    # Freeze before scoring the internal check. All metrics retain native origins.
    save(output / "state.json", state)
    data = hints.build(state)
    (output / "font.ttf").write_bytes(data)
    rows = runner.render(data, sorted(all_cases))
    scored = api.score(pages, rows)
    stats = score(rows, all_cases)
    save(
        output / "report.json",
        dict(
            schema="ascii-font-bootstrap-v1",
            plan_sha256=sha((output / "plan.json").read_bytes()),
            state_sha256=sha((output / "state.json").read_bytes()),
            font_sha256=sha(data),
            policies=policies,
            spacing=spacing_report,
            search=metrics({q: stats[q] for q in training}),
            internal_check=metrics({q: stats[q] for q in check}),
            pages=scored,
            production_ready=False,
            visible_glyphs=len(shapes),
            advance_glyphs=len(advances),
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("outlines", "fit"))
    parser.add_argument("sampling", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--outlines", type=Path)
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "outlines":
        recover(
            args.sampling,
            args.output,
            args.engine,
            args.cache,
            args.workers,
            args.resume,
        )
    else:
        require(args.outlines is not None, "fit requires frozen outlines")
        fit(args.sampling, args.outlines, args.output, args.engine, args.cache)
