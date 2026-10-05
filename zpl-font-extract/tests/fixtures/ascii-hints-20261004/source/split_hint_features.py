"""Separate disconnected feature runs without changing their initial placement.

Equal coordinates do not imply the same stem: an @ terminal and bowl can share
an X coordinate. Preserve all original reference points and coordinates, clone
their instructions for disconnected runs, then let fitting change them apart.
SCFS, IP and IUP semantics follow the OpenType instruction definitions:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

from copy import deepcopy
import itertools

import joint_hint_program as hints


def components(shape, group, axis):
    locations = {}
    offset = 0
    for contour in shape:
        for i in range(len(contour)):
            locations[offset + i] = (contour, i, offset)
        offset += len(contour)
    members = group["points"]
    origin = hints.value(shape, group, axis)
    parent = {i: i for i in members}

    def root(i):
        while parent[i] != i:
            i = parent[i]
        return i

    for a, b in itertools.combinations(members, 2):
        contour, i, offset = locations[a]
        other, j, other_offset = locations[b]
        if offset != other_offset:
            continue
        n = len(contour)
        if any(
            all(abs(contour[k % n][axis] - origin) <= 4 for k in range(start, end + 1))
            for start, end in ((i, i + (j - i) % n), (j, j + (i - j) % n))
        ):
            parent[root(b)] = root(a)
    groups = {}
    for i in members:
        groups.setdefault(root(i), []).append(i)
    return sorted(groups.values(), key=lambda g: (members[0] not in g, min(g)))


def split(state):
    result = deepcopy(state)
    for char, shape in result["shapes"].items():
        points = [p for contour in shape for p in contour]
        for axis, feature in enumerate(result["graph"][char]):
            old = deepcopy(feature["groups"])
            clones = {i: [i] for i in range(len(old))}
            for i, group in enumerate(old):
                if "split_from" in group:
                    continue
                pieces = components(shape, group, axis)
                if len(pieces) <= 1:
                    continue
                if len(feature["groups"]) + len(pieces) - 1 > 64:
                    raise ValueError("split feature graph exceeds budget")
                origin = hints.value(shape, group, axis)
                for j, members in enumerate(pieces):
                    node = dict(
                        group,
                        points=members,
                        span=[
                            min(points[p][1 - axis] for p in members),
                            max(points[p][1 - axis] for p in members),
                        ],
                        hint_origin=origin,
                        split_from=i,
                    )
                    if j == 0:
                        feature["groups"][i] = node
                    else:
                        clones[i].append(len(feature["groups"]))
                        feature["groups"].append(node)
                        programs = [result["programs"][char][axis]]
                        optical = result.get("optical_programs", {}).get(char)
                        if optical and optical["nodes"][axis] is not None:
                            programs.append(optical["nodes"][axis])
                        regime = result.get("regimes", {}).get(char, [None, None])[axis]
                        if regime:
                            programs.append(regime["nodes"])
                            if regime.get("smaller"):
                                programs.append(regime["smaller"]["nodes"])
                        for nodes in programs:
                            nodes.append(deepcopy(nodes[i]))
            for kind in ("stems", "counters"):
                if kind not in feature:
                    continue
                feature[kind] = [
                    dict(pair, low=a, high=b)
                    for pair in feature[kind]
                    for a in clones[pair["low"]]
                    for b in clones[pair["high"]]
                ]
    return result
