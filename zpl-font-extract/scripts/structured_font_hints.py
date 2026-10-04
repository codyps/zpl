"""Stroke/counter chains, anchor interpolation, and compact projected-size regimes.

Original geometry-derived proposals. IP and MPPEM semantics are defined by:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
At most one coarse small-size branch per glyph axis; no DELTA instructions,
per-size coordinate corrections or bitmap data. Shapes stay fixed.
"""

from copy import deepcopy
import json

import joint_hint_program as hints
from reconstruct_geometry import flatten, winding
from refine_font_features import extend

CUTOFFS = (16, 24, 32)


def initialize(baseline):
    state, _ = extend(baseline)
    for char, shape in state["shapes"].items():
        loops = [flatten(c) for c in shape]
        for axis, feature in enumerate(state["graph"][char]):
            groups = feature["groups"]
            counters = []
            for i, a in enumerate(groups):
                for j, b in enumerate(groups):
                    distance = b["value"] - a["value"]
                    lo, hi = max(a["span"][0], b["span"][0]), min(
                        a["span"][1], b["span"][1]
                    )
                    if not 32 <= distance <= 1024 or lo > hi:
                        continue
                    if all(
                        winding((v, s) if axis == 0 else (s, v), loops) == 0
                        for t in (0.25, 0.5, 0.75)
                        for v in [a["value"] + distance * t]
                        for u in (0.25, 0.5, 0.75)
                        for s in [lo + (hi - lo) * u]
                    ):
                        counters.append(
                            dict(low=i, high=j, width=distance, support=max(1, hi - lo))
                        )
            feature["counters"] = counters
    state["regimes"] = {c: [None, None] for c in state["shapes"]}
    if hints.build(state) != hints.build(baseline):
        raise ValueError("initial structural expansion changed the baseline font")
    return state


def axis_neighbors(state, char, axis, nodes, relational=True):
    """Atomic operations plus coordinated stroke-gap-stroke moves."""
    temp = deepcopy(state)
    temp["programs"][char][axis] = nodes
    for program in hints.neighbors(temp, char):
        if program[1 - axis] == temp["programs"][char][1 - axis]:
            yield program[axis]
    if not relational:
        return
    feature = state["graph"][char][axis]
    groups = feature["groups"]
    positions = [g["value"] for g in groups]
    for i, node in enumerate(nodes):
        if node and node["op"] == "link":
            for mode in hints.ROUNDS:
                candidate = deepcopy(nodes)
                candidate[i]["round"] = mode
                yield candidate
        left = [
            j for j, other in enumerate(nodes) if other and positions[j] < positions[i]
        ]
        right = [
            j for j, other in enumerate(nodes) if other and positions[j] > positions[i]
        ]
        if left and right:
            for a, b in {
                (
                    max(left, key=lambda j: positions[j]),
                    min(right, key=lambda j: positions[j]),
                ),
                (
                    min(left, key=lambda j: positions[j]),
                    max(right, key=lambda j: positions[j]),
                ),
            }:
                for mode in ("none", "grid"):
                    candidate = deepcopy(nodes)
                    candidate[i] = dict(op="interpolate", refs=[a, b], round=mode)
                    yield candidate
    for edge in feature["stems"] + feature["counters"]:
        for anchor, other in ((edge["low"], edge["high"]), (edge["high"], edge["low"])):
            if nodes[anchor] is None:
                continue
            for mode in ("none", "grid"):
                for minimum in (False, True):
                    candidate = deepcopy(nodes)
                    candidate[other] = dict(
                        op="relative", ref=anchor, round=mode, minimum=minimum
                    )
                    yield candidate
    for gap in feature["counters"]:
        for first in feature["stems"]:
            if first["high"] != gap["low"]:
                continue
            for second in feature["stems"]:
                if second["low"] != gap["high"]:
                    continue
                a, b, c, d = first["low"], first["high"], second["low"], second["high"]
                if len({a, b, c, d}) != 4:
                    continue
                for mode in ("grid", "half"):
                    for gap_round in ("none", "grid"):
                        candidate = deepcopy(nodes)
                        candidate[a] = dict(op="anchor", round=mode)
                        candidate[b] = dict(
                            op="link",
                            ref=a,
                            round="grid",
                            stem=hints.shared_index(
                                state["parameters"], axis, first["width"]
                            ),
                        )
                        candidate[c] = dict(
                            op="relative", ref=b, round=gap_round, minimum=True
                        )
                        candidate[d] = dict(
                            op="link",
                            ref=c,
                            round="grid",
                            stem=hints.shared_index(
                                state["parameters"], axis, second["width"]
                            ),
                        )
                        yield candidate


def neighbors(state, char, axis, structured):
    """Return whole axis alternatives; branch fallback always remains explicit."""
    targets = [None] + list(CUTOFFS) if structured else [None]
    existing = state.get("regimes", {}).get(char, [None, None])[axis]
    for cutoff in targets:
        nodes = (
            existing["nodes"]
            if existing and cutoff == existing["limit"]
            else state["programs"][char][axis]
        )
        seen = {json.dumps(nodes, sort_keys=True)}
        for alternative in axis_neighbors(
            state, char, axis, nodes, relational=structured
        ):
            key = json.dumps(alternative, sort_keys=True)
            if key in seen:
                continue
            seen.add(key)
            try:
                hints.ordered(alternative, state["graph"][char][axis])
            except ValueError:
                continue
            candidate = deepcopy(state)
            if cutoff is None:
                candidate["programs"][char][axis] = alternative
            else:
                candidate["regimes"][char][axis] = dict(limit=cutoff, nodes=alternative)
            yield candidate


def complexity(state, baseline):
    """Description-length cost for edits, dependencies and extra branches."""
    edits = 0
    for c, axes in state["programs"].items():
        for axis, nodes in enumerate(axes):
            old = baseline["programs"][c][axis]
            edits += sum(
                node != (old[i] if i < len(old) else None)
                for i, node in enumerate(nodes)
            )
            regime = state.get("regimes", {}).get(c, [None, None])[axis]
            if regime:
                edits += 2 + sum(
                    a != b for a, b in zip(nodes, regime["nodes"], strict=True)
                )
                edits += sum(
                    n is not None and n["op"] in ("relative", "interpolate")
                    for n in regime["nodes"]
                )
    return edits
