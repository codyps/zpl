"""Target-driven, coordinated fitting of scalable TrueType hint programs.

Learn bounded rounding thresholds and fractional edge positions shared across
sizes. Whole-axis templates cross barriers that single-feature descent misses.
Every score executes the original renderer; old checks are scored after freeze.
TrueType F26Dot6, ROUND, SCFS and IUP semantics:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import argparse
from collections import defaultdict
from copy import deepcopy
import json
from itertools import chain, product
import math
from pathlib import Path

import examine_font_accuracy as examination
from fit_structured_hints import Oracle, Objective, aggregate, error
import joint_hint_program as hints
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha
import structured_font_hints as structure
import split_hint_features

# Ten is the measured ZD621 minimum effective ppem, shared by smaller requests.
OPTICAL_CUTOFFS = (10, 12, 16, 24, 32)
SMALLER_CUTOFFS = (10, 12, 16, 24)
COARSE_CUTOFFS = (12, 16, 24, 32)


def programs(state):
    for char, axes in state["programs"].items():
        optical = state.get("optical_programs", {}).get(char)
        if optical:
            yield from (nodes for nodes in optical["nodes"] if nodes is not None)
        for axis, nodes in enumerate(axes):
            yield nodes
            regime = state.get("regimes", {}).get(char, [None, None])[axis]
            if regime:
                yield regime["nodes"]
                if regime.get("smaller"):
                    yield regime["smaller"]["nodes"]


def target_score(stats, objective, state, baseline):
    """Minimax target, then remaining deficits, then accuracy/model economy.

    The tuple is lexicographic: aggregate quality or a smaller program cannot
    compensate for a worse minimum IoU or defeat a feasible all-cases result.
    """
    _, violations = objective.measure(stats)
    if violations:
        return (float("inf"),) * 3
    deficits = [max(0.0, error(s) - 0.1) for s in stats.values()]
    worst = sorted(deficits, reverse=True)[: max(1, math.ceil(len(deficits) / 4))]
    buckets = [aggregate(stats[q] for q in keys) for keys in objective.buckets.values()]
    numeric = sum(
        abs(n.get("shift", 0)) / 64
        + abs(n.get("phase", 0)) / 32
        + abs(n.get("design", 0)) / 64
        + bool(n.get("fade"))
        for nodes in programs(state)
        for n in nodes
        if n
    )
    deficits_loss = (
        sum(d * d for d in deficits) / len(deficits)
        + sum(d * d for d in worst) / len(worst)
        + sum(max(0.0, v - 0.1) ** 2 for b in buckets for v in b) / len(buckets)
    )
    economy = (
        0.05 * sum(sum(b) for b in buckets) / len(buckets)
        + 1e-5 * structure.complexity(state, baseline)
        + 1e-6 * numeric
    )
    return tuple(round(v, 12) for v in (max(deficits), deficits_loss, economy))


def axis_templates(state, char, axis):
    feature = state["graph"][char][axis]
    for mode in hints.ROUNDS:
        yield [dict(op="anchor", round=mode) for _ in feature["groups"]]
        for i, group in enumerate(feature["groups"]):
            if group.get("shoulder"):
                continue
            sparse = [None] * len(feature["groups"])
            sparse[i] = dict(op="anchor", round=mode)
            yield sparse
        for high in (False, True):
            for center in (False, True):
                nodes = [None] * len(feature["groups"])
                for stem in feature["stems"]:
                    a, b = (
                        (stem["high"], stem["low"])
                        if high
                        else (stem["low"], stem["high"])
                    )
                    width = hints.shared_index(state["parameters"], axis, stem["width"])
                    nodes[a] = (
                        dict(op="center", other=b, stem=width, round=mode)
                        if center
                        else dict(op="anchor", round=mode)
                    )
                    nodes[b] = dict(op="link", ref=a, stem=width, round="grid")
                yield nodes
    yield [None] * len(feature["groups"])


def adjusted(node, field, value):
    node = dict(node)
    if value:
        node[field] = value
    else:
        node.pop(field, None)
    return node


def node_proposals(state, char, axis, nodes, step, structural):
    if structural:
        yield from axis_templates(state, char, axis)
        yield from structure.axis_neighbors(state, char, axis, nodes)
        for pair in state["graph"][char][axis]["stems"]:
            for low_round in hints.ROUNDS:
                for high_round in hints.ROUNDS:
                    alternative = deepcopy(nodes)
                    alternative[pair["low"]] = dict(op="anchor", round=low_round)
                    alternative[pair["high"]] = dict(op="anchor", round=high_round)
                    yield alternative
    for i, node in enumerate(nodes):
        if node is None:
            if structural and state["graph"][char][axis]["groups"][i].get("shoulder"):
                for end in (16, 24, 32):
                    for measure in ("axis", "y", "max", "min"):
                        for delta in (-step, step):
                            alternative = deepcopy(nodes)
                            alternative[i] = dict(
                                op="adjust",
                                round="none",
                                shift=delta,
                                fade=dict(end=end, measure=measure),
                            )
                            yield alternative
            continue
        if structural and (node.get("phase", 0) or node.get("shift", 0)):
            for end in (16, 24, 32, 48):
                for measure in ("axis", "x", "y", "min", "max"):
                    alternative = deepcopy(nodes)
                    alternative[i]["fade"] = dict(end=end, measure=measure)
                    yield alternative
            if node.get("fade"):
                alternative = deepcopy(nodes)
                alternative[i].pop("fade")
                yield alternative
        for field, bound in (("phase", 32), ("shift", 64), ("design", 64)):
            if field == "phase" and node["op"] in ("interpolate", "adjust"):
                continue
            if field == "design" and node["op"] != "anchor":
                continue
            for delta in (-step, step):
                value = node.get(field, 0) + delta
                if abs(value) > bound:
                    continue
                alternative = deepcopy(nodes)
                alternative[i] = adjusted(node, field, value)
                yield alternative
    # Shared threshold/translation proposals coordinate dependent edges.
    for field, bound in (("phase", 32), ("shift", 64)):
        for delta in (-step, step):
            alternative = deepcopy(nodes)
            for i, node in enumerate(nodes):
                if node and not (
                    field == "phase" and node["op"] in ("interpolate", "adjust")
                ):
                    if field == "shift" and node["op"] in (
                        "link",
                        "relative",
                        "interpolate",
                    ):
                        continue
                    value = node.get(field, 0) + delta
                    if abs(value) <= bound:
                        alternative[i] = adjusted(node, field, value)
            yield alternative
    for i, node in enumerate(nodes):
        if not node or node["op"] not in ("link", "relative"):
            continue
        anchor = node["ref"]
        for field, bound in (("shift", 64), ("phase", 32)):
            if field == "phase" and nodes[anchor]["op"] in ("interpolate", "adjust"):
                continue
            for delta in (-step, step):
                a, b = nodes[anchor].get(field, 0) + delta, node.get(field, 0) - delta
                if max(abs(a), abs(b)) <= bound:
                    alternative = deepcopy(nodes)
                    alternative[anchor] = adjusted(nodes[anchor], field, a)
                    alternative[i] = adjusted(node, field, b)
                    yield alternative


def proposals(state, char, axis, step, structural):
    """At most two coarse branches; parameters are shared within each range."""
    regime = state.get("regimes", {}).get(char, [None, None])[axis]
    locations = [(0, None, state["programs"][char][axis])]
    optical = state.get("optical_programs", {}).get(char)
    if optical and optical["nodes"][axis] is not None:
        locations.append((3, None, optical["nodes"][axis]))
    if regime:
        locations.append((1, None, regime["nodes"]))
        smaller = regime.get("smaller")
        if smaller:
            locations.append((2, smaller["limit"], smaller["nodes"]))
            if structural:
                locations.extend(
                    (2, cutoff, smaller["nodes"])
                    for cutoff in SMALLER_CUTOFFS
                    if cutoff < regime["limit"] and cutoff != smaller["limit"]
                )
        elif structural:
            locations.extend(
                (2, cutoff, regime["nodes"])
                for cutoff in SMALLER_CUTOFFS
                if cutoff < regime["limit"]
            )
    for branch, cutoff, nodes in locations:
        for alternative in node_proposals(state, char, axis, nodes, step, structural):
            try:
                hints.ordered(alternative, state["graph"][char][axis])
            except ValueError:
                continue
            result = deepcopy(state)
            if branch == 0:
                result["programs"][char][axis] = alternative
            elif branch == 1:
                result["regimes"][char][axis]["nodes"] = alternative
            elif branch == 3:
                result["optical_programs"][char]["nodes"][axis] = alternative
            else:
                result["regimes"][char][axis]["smaller"] = dict(
                    (regime.get("smaller") or {}), limit=cutoff, nodes=alternative
                )
            yield result
    if structural:
        if optical:
            for measure in ("axis", "x", "y", "min", "max"):
                result = deepcopy(state)
                result["optical_programs"][char].setdefault("measures", ["max", "max"])[
                    axis
                ] = measure
                yield result
            result = deepcopy(state)
            result["optical_programs"][char]["nodes"][axis] = None
            if all(n is None for n in result["optical_programs"][char]["nodes"]):
                del result["optical_programs"][char]
            yield result
            if axis == 0:
                for cutoff in OPTICAL_CUTOFFS:
                    result = deepcopy(state)
                    result["optical_programs"][char]["limit"] = cutoff
                    yield result
        if regime:
            for measure in ("axis", "x", "y", "min", "max"):
                for smaller in (False, True):
                    if smaller and not regime.get("smaller"):
                        continue
                    result = deepcopy(state)
                    r = result["regimes"][char][axis]
                    if smaller:
                        r = r["smaller"]
                    if measure == "axis":
                        r.pop("measure", None)
                    else:
                        r["measure"] = measure
                    yield result
        for cutoff in COARSE_CUTOFFS:
            if regime:
                if regime.get("smaller") and cutoff <= regime["smaller"]["limit"]:
                    continue
                result = deepcopy(state)
                result["regimes"][char][axis]["limit"] = cutoff
                yield result
            else:
                seed = state["programs"][char][axis]
                # An identical new branch only adds model cost, so greedy
                # search would never accept it and then refine it. Introduce
                # the range together with a local edit, retaining the fallback.
                local = (
                    program[axis]
                    for program in hints.neighbors(state, char)
                    if program[1 - axis] == state["programs"][char][1 - axis]
                )
                for nodes in chain(
                    axis_templates(state, char, axis),
                    local,
                    node_proposals(state, char, axis, seed, step, False),
                ):
                    try:
                        hints.ordered(nodes, state["graph"][char][axis])
                    except ValueError:
                        continue
                    result = deepcopy(state)
                    result.setdefault("regimes", {}).setdefault(char, [None, None])[
                        axis
                    ] = dict(limit=cutoff, nodes=nodes)
                    yield result
                    # Introduce a useful medium-size template while retaining
                    # the original small-size behavior in the same proposal.
                    result = deepcopy(state)
                    result["programs"][char][axis] = nodes
                    result.setdefault("regimes", {}).setdefault(char, [None, None])[
                        axis
                    ] = dict(
                        limit=cutoff, nodes=deepcopy(state["programs"][char][axis])
                    )
                    yield result
        if axis == 0:
            result = deepcopy(state)
            independent = set(result.get("independent_axes", []))
            independent.symmetric_difference_update({char})
            result["independent_axes"] = sorted(independent)
            yield result


def metrics(stats):
    errors = [error(s) for s in stats.values()]
    return dict(
        cases=len(errors),
        at_least_90=sum(e <= 0.1 + 1e-12 for e in errors),
        passes_target=all(e <= 0.1 + 1e-12 for e in errors),
        worst_iou=1 - max(errors),
        mean_iou=1 - sum(errors) / len(errors),
        pooled_iou=1 - aggregate(stats.values())[1],
    )


def screen_reject(stats, baseline):
    """A single measured violation proves the complete proposal inadmissible."""
    return any(
        (not baseline[q]["xor"] and v["xor"])
        or error(v) > error(baseline[q]) + 0.06 + 1e-12
        for q, v in stats.items()
    )


def small_program(state, char, axis, nodes, cutoff, measure="max"):
    """Overlay a small-square program, preserving all normal branches outside."""
    result = deepcopy(state)
    optical = result.setdefault("optical_programs", {})
    if char not in optical or optical[char]["limit"] != cutoff:
        optical[char] = dict(limit=cutoff, nodes=[None, None])
    optical[char]["nodes"][axis] = deepcopy(nodes)
    optical[char].setdefault("measures", ["max", "max"])[axis] = measure
    return result


def joint_repair(state, oracle, objective, baseline, beam=12, measures=("max",)):
    """Rank axis proposals locally, then accept only fully constrained pairs.

    Local ranks deliberately permit intermediate regressions: they nominate
    paired moves across a barrier, never authorize a model change themselves.
    Only training pixels participate. No exact-size branch is introduced.
    """
    current = oracle.stats(state)
    characters = sorted(
        oracle.keys, key=lambda c: (-max(error(current[q]) for q in oracle.keys[c]), c)
    )[:2]
    stages = []
    for char in characters:
        for measure, cutoff in product(measures, OPTICAL_CUTOFFS):
            require(measure in ("max", "min", "x", "y"), "invalid joint search region")
            keys = [
                q
                for q in oracle.keys[char]
                if hints.regime_matches(
                    dict(limit=cutoff, measure=measure),
                    0,
                    [math.floor(pipeline.ppem(d) + 0.5) for d in q[:2]],
                )
            ]
            if not keys:
                continue
            alternatives = []
            for axis in (0, 1):
                active = state["programs"][char][axis]
                regime = state.get("regimes", {}).get(char, [None, None])[axis]
                if regime and regime["limit"] >= cutoff:
                    active = regime["nodes"]
                    if regime.get("smaller") and regime["smaller"]["limit"] >= cutoff:
                        active = regime["smaller"]["nodes"]
                optical = state.get("optical_programs", {}).get(char)
                if (
                    optical
                    and optical["limit"] >= cutoff
                    and optical["nodes"][axis] is not None
                ):
                    active = optical["nodes"][axis]
                seeds = [active, state["programs"][char][axis]]
                old = baseline.get("regimes", {}).get(char, [None, None])[axis]
                previous = [baseline["programs"][char][axis]]
                if old:
                    previous.append(old["nodes"])
                    if old.get("smaller"):
                        previous.append(old["smaller"]["nodes"])
                for original in previous:
                    nodes = deepcopy(original)
                    for group in state["graph"][char][axis]["groups"][len(nodes) :]:
                        source = group.get("split_from")
                        nodes.append(
                            deepcopy(nodes[source]) if source is not None else None
                        )
                    seeds.append(nodes)
                seeds = list({json.dumps(n, sort_keys=True): n for n in seeds}.values())
                ranked, seen = [], set()
                for nodes in chain(
                    seeds,
                    axis_templates(state, char, axis),
                    chain.from_iterable(
                        node_proposals(state, char, axis, seed, 16, True)
                        for seed in seeds
                    ),
                ):
                    identity = json.dumps(nodes, sort_keys=True)
                    if identity in seen:
                        continue
                    seen.add(identity)
                    try:
                        hints.ordered(nodes, state["graph"][char][axis])
                    except ValueError:
                        continue
                    candidate = small_program(state, char, axis, nodes, cutoff, measure)
                    scores = oracle.stats(candidate, keys)
                    errors = [error(s) for s in scores.values()]
                    rank = (
                        sum(e * e for e in errors) / len(errors),
                        max(errors),
                        identity,
                    )
                    pattern = tuple(
                        (scores[q]["under"], scores[q]["over"], scores[q]["union"])
                        for q in keys
                    )
                    ranked.append((rank, nodes, pattern))
                ranked.sort(key=lambda item: item[0])
                # Avoid spending the entire beam on numerically different
                # instructions producing the same per-case error pattern.
                selected, patterns = [], set()
                for _, nodes, pattern in ranked:
                    if pattern in patterns:
                        continue
                    patterns.add(pattern)
                    selected.append(nodes)
                    if len(selected) == beam:
                        break
                alternatives.append(selected)
            before = target_score(current, objective, state, baseline)
            best, best_score, best_stats = state, before, current
            for x in alternatives[0]:
                for y in alternatives[1]:
                    candidate = small_program(state, char, 0, x, cutoff, measure)
                    candidate = small_program(candidate, char, 1, y, cutoff, measure)
                    scores = dict(current)
                    scores.update(oracle.stats(candidate, oracle.keys[char]))
                    score = target_score(scores, objective, candidate, baseline)
                    if score < best_score:
                        best, best_score, best_stats = candidate, score, scores
            state, current = best, best_stats
            row = dict(
                stage="joint-repair",
                glyph=char,
                cutoff=cutoff,
                measure=measure,
                before=before,
                after=best_score,
                evaluations=len(oracle.engine.seen),
                **metrics(current),
            )
            stages.append(row)
            print(json.dumps(row), flush=True)
    return state, stages


def fit(args):
    _, _, inputs, references, pages, train, check = examination.development(
        args.model, args.captures
    )
    old = json.loads((args.seed / "report.json").read_text())
    data = (args.seed / "candidates.json").read_bytes()
    require(sha(data) == old["candidates_sha256"], "seed candidates changed")
    seed_variant = getattr(args, "seed_variant", "structured_robust")
    baseline = json.loads(data)[seed_variant]
    require(
        sha(hints.build(baseline)) == old["variants"][seed_variant]["font_sha256"],
        "seed compiler output changed",
    )
    warm_path = getattr(args, "warm_start", None)
    warm_data = warm_path.read_bytes() if warm_path else None
    warm = json.loads(warm_data) if warm_data else baseline
    initial = split_hint_features.split(
        structure.curve_shoulders(structure.curved_stems(warm))
    )
    args.output.mkdir(parents=True, exist_ok=False)
    if warm_path:
        (args.output / "warm-start.json").write_bytes(warm_data)
    script_names = (
        Path(__file__).name,
        "joint_hint_program.py",
        "fit_structured_hints.py",
        "structured_font_hints.py",
        "reconstruct_geometry.py",
        "refine_font_features.py",
        "reconstruct_font.py",
        "font0_hint_model.py",
        "font_probe.py",
        "split_hint_features.py",
    )
    source = args.output / "source"
    source.mkdir()
    for name in script_names:
        (source / name).write_bytes(Path(__file__).with_name(name).read_bytes())
    engine = pipeline.Engine(
        args.engine, args.cache, getattr(args, "max_evaluations", 100000)
    )
    require(engine.digest == inputs["engine_sha256"], "fitting engine changed")
    plan = dict(
        schema="structured-font-fit-v1",
        seed_sha256=sha(data),
        seed_variant=seed_variant,
        warm_start_sha256=sha(warm_data) if warm_data else None,
        model_sha256=sha((args.model / "model.json").read_bytes()),
        inputs_sha256=sha((args.model / "inputs.json").read_bytes()),
        captures=[
            dict(
                path=str(p),
                manifest_sha256=sha((p / "manifest.json").read_bytes()),
                capture_sha256=sha((p / "zd621/capture.json").read_bytes()),
            )
            for p in args.captures
        ],
        engine_sha256=engine.digest,
        printer=inputs["source"]["printer"],
        search_queries=sorted(train),
        internal_check_queries=sorted(check),
        target_iou=0.9,
        objective="lexicographic worst deficit, remaining squared deficits, aggregate error and model cost",
        sweeps=args.sweeps,
        max_evaluations=engine.budget,
        steps=[16, 8, 4],
        local_steps=4,
        glyph_order="worst training-case IoU first at each sweep",
        joint_repair=dict(
            glyphs=2,
            cutoffs=list(OPTICAL_CUTOFFS),
            beam=12,
            baseline_restarts=True,
            measures=getattr(args, "joint_measures", ["max"]),
        ),
        maximum_branches_per_axis=2,
        additional_optical_program=dict(
            cutoffs=list(OPTICAL_CUTOFFS),
            axis_measures=["axis", "x", "y", "min", "max"],
        ),
        skip_joint_repair=getattr(args, "skip_joint_repair", False),
        smaller_cutoffs=list(SMALLER_CUTOFFS),
        coarse_cutoffs=list(COARSE_CUTOFFS),
        feature_expansion=["offset curve stems", "curve shoulders"],
        split_disconnected_features=True,
        optical_fades=dict(
            full_ppem=10, end_ppem=[16, 24, 32, 48], post_interpolation=True
        ),
        regime_measures=["axis", "x", "y", "min", "max"],
        screening="up to 24 exact/worst cases per changed glyph, batches of 8; disable after 128 proposals if fewer than half reject; score every surviving candidate in full",
        selection="search only; target every case; freeze before old-check and fresh audits",
        scripts={n: sha((source / n).read_bytes()) for n in script_names},
    )
    save(args.output / "plan.json", plan)
    # Splitting feature groups changes bytecode order, so verify actual raster
    # equivalence rather than relying on instruction-byte equality.
    warm_rows = engine.render(hints.build(warm), sorted(train))
    initial_rows = engine.render(hints.build(initial), sorted(train))
    require(
        all(
            pipeline.pixels(warm_rows[q]) == pipeline.pixels(initial_rows[q])
            for q in train
        ),
        "feature split changed initial training rasters",
    )
    oracle = Oracle(engine, {q: references[q] for q in sorted(train)}, initial)
    objective = Objective(oracle.stats(baseline), "robust")
    critical = {
        c: sorted(
            keys,
            key=lambda q: (
                objective.baseline[q]["xor"] != 0,
                -error(objective.baseline[q]),
                q[0] * q[1],
                q,
            ),
        )[:24]
        for c, keys in oracle.keys.items()
    }
    state = initial
    require(
        math.isfinite(target_score(oracle.stats(state), objective, state, baseline)[0]),
        "warm start fails original training constraints",
    )
    stages = []
    if not plan["skip_joint_repair"]:
        state, stages = joint_repair(
            state,
            oracle,
            objective,
            baseline,
            measures=plan["joint_repair"]["measures"],
        )
    save(args.output / "progress.json", stages)
    save(args.output / "checkpoint.json", state)
    for sweep in range(args.sweeps):
        step = plan["steps"][min(sweep // 2, len(plan["steps"]) - 1)]
        sweep_stats = oracle.stats(state)
        characters = sorted(
            state["shapes"],
            key=lambda c: (-max(error(sweep_stats[q]) for q in oracle.keys[c]), c),
        )
        for char in characters:
            for axis in (0, 1):
                current_stats = oracle.stats(state)
                before = target_score(current_stats, objective, state, baseline)
                seen, best_score = set(), before
                screened = 0
                screening = True
                for local_step in range(plan["local_steps"]):
                    best = state
                    for candidate in proposals(
                        state, char, axis, step, sweep % 2 == 0 and local_step == 0
                    ):
                        identity = json.dumps(
                            [
                                candidate["programs"][char][axis],
                                candidate["regimes"][char][axis],
                                char in candidate.get("independent_axes", []),
                                candidate.get("optical_programs", {}).get(char),
                            ],
                            sort_keys=True,
                        )
                        if identity in seen:
                            continue
                        seen.add(identity)
                        if len(seen) >= 128 and screened * 2 < len(seen):
                            screening = False
                        if screening and any(
                            screen_reject(
                                oracle.stats(candidate, critical[char][i : i + 8]),
                                objective.baseline,
                            )
                            for i in range(0, len(critical[char]), 8)
                        ):
                            screened += 1
                            continue
                        candidate_stats = dict(current_stats)
                        candidate_stats.update(
                            oracle.stats(candidate, oracle.keys[char])
                        )
                        score = target_score(
                            candidate_stats, objective, candidate, baseline
                        )
                        if score < best_score:
                            best, best_score, best_stats = (
                                candidate,
                                score,
                                candidate_stats,
                            )
                        if len(seen) % 500 == 0:
                            print(
                                json.dumps(
                                    dict(
                                        sweep=sweep,
                                        glyph=char,
                                        axis=axis,
                                        proposals=len(seen),
                                        best=best_score,
                                        evaluations=len(engine.seen),
                                    )
                                ),
                                flush=True,
                            )
                    if best is state:
                        break
                    state = best
                    current_stats = best_stats
                row = dict(
                    sweep=sweep,
                    glyph=char,
                    axis=axis,
                    step=step,
                    before=before,
                    after=best_score,
                    proposals=len(seen),
                    screened=screened,
                    evaluations=len(engine.seen),
                    **metrics(oracle.stats(state)),
                )
                stages.append(row)
                save(args.output / "progress.json", stages)
                save(args.output / "checkpoint.json", state)
                print(json.dumps(row), flush=True)
    candidates = dict(baseline=baseline, targeted=state)
    save(args.output / "candidates.json", candidates)
    for label, state in candidates.items():
        (args.output / f"{label}.ttf").write_bytes(hints.build(state))
    report = dict(
        schema=plan["schema"],
        plan_sha256=sha((args.output / "plan.json").read_bytes()),
        candidates_sha256=sha((args.output / "candidates.json").read_bytes()),
        designated="targeted",
        production_ready=False,
        variants={},
    )
    for label, state in candidates.items():
        data = hints.build(state)
        rows = engine.render(data, sorted(references))
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
        require(
            all(
                all(keyed[q][k] == v[k] for k in ("under", "over", "xor", "union"))
                for q, v in oracle.stats(state).items()
            ),
            "cache audit failed",
        )
        report["variants"][label] = dict(
            font_sha256=sha(data),
            cache_audit_exact=True,
            search=examination.breakdown([keyed[q] for q in sorted(train)]),
            previous_internal_check=examination.breakdown(
                [keyed[q] for q in sorted(check)]
            ),
            search_target=metrics({q: keyed[q] for q in train}),
            check_target=metrics({q: keyed[q] for q in check}),
            pages=scored,
        )
        save(args.output / "report.json", report)
        print(
            json.dumps(
                dict(
                    label=label,
                    search=report["variants"][label]["search_target"],
                    check=report["variants"][label]["check_target"],
                )
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("model", "seed", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--captures", type=Path, nargs="+", required=True)
    parser.add_argument("--sweeps", type=int, default=6)
    parser.add_argument("--max-evaluations", type=int, default=100000)
    parser.add_argument(
        "--joint-measures", nargs="+", choices=("max", "min", "x", "y"), default=["max"]
    )
    parser.add_argument(
        "--skip-joint-repair",
        action="store_true",
        help="Refine an already repaired warm start without repeating the joint search",
    )
    parser.add_argument("--seed-variant", default="structured_robust")
    parser.add_argument("--warm-start", type=Path)
    fit(parser.parse_args())
