"""Nominate coupled hint programs with axis-projected foreground ink guides.

Projection is a search surrogate, never an acceptance metric. A glyph displaced
in both directions can have worse 2D IoU after repairing either direction alone.
Column/row ink guides separate that barrier; final pairs execute the original
renderer and must pass every historical and incumbent guard at native origins.
"""

from copy import deepcopy
from itertools import product
import json
import math

from fit_font_target import axis_templates, copy_hint_state, metrics, target_score
import joint_hint_program as hints
import reconstruct_font as pipeline


def active_branch(state, char, axis, query):
    projected = [math.floor(pipeline.ppem(d) + 0.5) for d in query[:2]]
    optical = state.get("optical_programs", {}).get(char)
    if (
        optical
        and optical["nodes"][axis] is not None
        and hints.regime_matches(
            dict(
                limit=optical["limit"],
                measure=optical.get("measures", ["max", "max"])[axis],
            ),
            axis,
            projected,
        )
    ):
        return "optical"
    regime = state.get("regimes", {}).get(char, [None, None])[axis]
    if regime and hints.regime_matches(regime, axis, projected):
        if regime.get("smaller") and hints.regime_matches(
            regime["smaller"], axis, projected
        ):
            return "smaller"
        return "regime"
    return "main"


def get_nodes(state, char, axis, branch):
    if branch == "main":
        return state["programs"][char][axis]
    if branch == "optical":
        return state["optical_programs"][char]["nodes"][axis]
    regime = state["regimes"][char][axis]
    return (regime["smaller"] if branch == "smaller" else regime)["nodes"]


def replace_nodes(state, char, axis, branch, nodes):
    result = copy_hint_state(state, char)
    target = get_nodes(result, char, axis, branch)
    target[:] = deepcopy(nodes)
    return result


def templates(state, char, axis, branch):
    seeds = [get_nodes(state, char, axis, branch)]
    seeds.extend(axis_templates(state, char, axis))
    seen = set()
    for nodes, phase, shift in product(seeds, (0, -32, -16, 16, 32), (0, -32, 32)):
        candidate = deepcopy(nodes)
        for node in candidate:
            if node is None:
                continue
            if node["op"] not in ("interpolate", "adjust"):
                if phase:
                    node["phase"] = phase
                else:
                    node.pop("phase", None)
            if node["op"] in ("anchor", "zone", "center"):
                if shift:
                    node["shift"] = shift
                else:
                    node.pop("shift", None)
        key = json.dumps(candidate, sort_keys=True)
        if key in seen:
            continue
        seen.add(key)
        try:
            hints.ordered(candidate, state["graph"][char][axis])
        except ValueError:
            continue
        yield candidate


def repair(state, baseline, oracle, constraints, beam=16, rounds=2, callback=None):
    pipeline.require(
        oracle.projections is not None, "projection search needs an enabled oracle"
    )
    (char,) = state["shapes"]
    current = oracle.stats(state)
    stages = []
    for depth in range(rounds):
        critical = min(
            current, key=lambda q: (-current[q]["xor"] / max(1, current[q]["union"]), q)
        )
        branches = [active_branch(state, char, axis, critical) for axis in (0, 1)]
        pools = []
        evaluated = 0
        for axis, branch in enumerate(branches):
            ranked = []
            for nodes in templates(state, char, axis, branch):
                candidate = replace_nodes(state, char, axis, branch, nodes)
                oracle.stats(candidate)
                evaluated += 1
                errors = [row[0][axis] for row in oracle.last_projections.values()]
                rank = (
                    max(errors),
                    sum(e * e for e in errors) / len(errors),
                    json.dumps(nodes, sort_keys=True),
                )
                # Full native raster identities, not equal error counts, define
                # duplicates. Keep two parameterizations of a raster plateau.
                signature = tuple(
                    v[1] for q, v in sorted(oracle.last_projections.items())
                )
                ranked.append((rank, nodes, signature))
            pool = [get_nodes(state, char, axis, branch)]
            seen = {}
            for _, nodes, signature in sorted(ranked, key=lambda r: r[0]):
                if seen.get(signature, 0) >= 2 or nodes in pool:
                    continue
                seen[signature] = seen.get(signature, 0) + 1
                pool.append(nodes)
                if len(pool) >= beam:
                    break
            pools.append(pool)
        before = target_score(current, constraints, state, baseline)
        best, best_stats, best_score = state, current, before
        accepted = 0
        for x, y in product(*pools):
            candidate = replace_nodes(state, char, 0, branches[0], x)
            candidate = replace_nodes(candidate, char, 1, branches[1], y)
            stats = oracle.stats(candidate)
            evaluated += 1
            score = target_score(stats, constraints, candidate, baseline)
            if score < best_score:
                best, best_stats, best_score = candidate, stats, score
                accepted += 1
        state, current = best, best_stats
        row = dict(
            stage="projection",
            depth=depth,
            critical_query=critical,
            branches=branches,
            evaluated=evaluated,
            accepted=accepted,
            before=before,
            after=best_score,
            **metrics(current)
        )
        stages.append(row)
        if callback:
            callback(state, row)
        if best_score >= before or metrics(current)["passes_target"]:
            break
    pipeline.require(
        not constraints.measure(current)[1], "projection repair violated constraints"
    )
    return state, current, stages
