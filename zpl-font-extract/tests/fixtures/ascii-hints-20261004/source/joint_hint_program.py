"""Original bounded per-feature hint language and TrueType compiler.

Operations: anchor, zone, center a stem, link an edge, and leave untouched.
References are an acyclic graph; only measured stem pairs can be linked.
SCFS, GC, RCVT, IUP, ROUND and rounding-state semantics follow OpenType:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

from copy import deepcopy
import statistics
import struct

import font_probe

ROUNDS = {"grid": 0x18, "half": 0x19, "floor": 0x7D, "ceil": 0x7C}


def value(shape, group, axis):
    if "hint_origin" in group:
        return group["hint_origin"]
    points = [p for contour in shape for p in contour]
    return round(statistics.median({points[i][axis] for i in group["points"]}))


def shared_index(parameters, axis, distance):
    widths = parameters["widths"][axis]
    if not widths:
        return None
    index = min(range(len(widths)), key=lambda i: abs(widths[i] - abs(distance)))
    return index if abs(widths[index] - abs(distance)) <= 32 else None


def initialize(model):
    """Translate the prior whole-axis policies without changing their meaning."""
    parameters = dict(
        widths=deepcopy(model["inferred"]["shared_widths"]),
        zones=deepcopy(model["inferred"]["zones"]),
        cutin=model["cutin"],
        limit=model["inferred"]["hint_ppem_limit"],
    )
    graph = deepcopy(model["inferred"]["graph"])
    programs = {}
    for char, axes in graph.items():
        program = []
        for axis, feature in enumerate(axes):
            groups = feature["groups"]
            nodes = [None] * len(groups)
            policy = model["policies"][char][axis]
            if policy in ("grid", "zones"):
                for i, group in enumerate(groups):
                    zone = (
                        min(
                            range(len(parameters["zones"])),
                            key=lambda j: abs(parameters["zones"][j] - group["value"]),
                        )
                        if axis == 1 and parameters["zones"]
                        else None
                    )
                    if (
                        policy == "zones"
                        and zone is not None
                        and abs(parameters["zones"][zone] - group["value"]) <= 48
                    ):
                        nodes[i] = dict(op="zone", zone=zone, round="grid")
                    else:
                        nodes[i] = dict(op="anchor", round="grid")
            elif policy != "none":
                for pair in feature["stems"]:
                    low, high = pair["low"], pair["high"]
                    stem = shared_index(parameters, axis, pair["width"])
                    anchor, other = (
                        (high, low) if policy == "stems-high" else (low, high)
                    )
                    nodes[anchor] = (
                        dict(op="center", other=other, stem=stem, round="grid")
                        if policy == "stems-center"
                        else dict(op="anchor", round="grid")
                    )
                    nodes[other] = dict(op="link", ref=anchor, stem=stem, round="grid")
            program.append(nodes)
        programs[char] = program
    return dict(
        shapes=deepcopy(model["shapes"]),
        graph=graph,
        programs=programs,
        parameters=parameters,
        zone_origins=deepcopy(parameters["zones"]),
    )


def ordered(nodes, feature):
    if len(nodes) != len(feature["groups"]) or len(nodes) > 64:
        raise ValueError("invalid feature-program dimensions")
    pairs = {frozenset((p["low"], p["high"])) for p in feature["stems"]}
    counters = {frozenset((p["low"], p["high"])) for p in feature.get("counters", [])}
    pending = {i for i, node in enumerate(nodes) if node is not None}
    dependencies = {}
    for i in pending:
        node = nodes[i]
        if node.get("op") not in (
            "anchor",
            "zone",
            "link",
            "center",
            "relative",
            "interpolate",
            "adjust",
        ) or not (
            node.get("round") in ROUNDS
            or (
                node.get("op") in ("relative", "interpolate", "adjust")
                and node.get("round") == "none"
            )
        ):
            raise ValueError("invalid hint operation")
        for name, bound in (("phase", 32), ("shift", 64), ("design", 64)):
            if not isinstance(node.get(name, 0), int) or abs(node.get(name, 0)) > bound:
                raise ValueError("hint adjustment exceeds its shared-size bound")
        if node.get("design", 0) and node["op"] != "anchor":
            raise ValueError("design adjustment requires an anchor")
        if node.get("phase", 0) and node["op"] == "interpolate":
            raise ValueError("interpolation has no distance-rounding phase")
        if node["op"] == "adjust" and (
            node.get("phase", 0) or node["round"] != "none" or not node.get("shift", 0)
        ):
            raise ValueError("post-interpolation adjustment requires a nonzero shift")
        if "fade" in node:
            fade = node["fade"]
            if fade.get("end") not in (16, 24, 32, 48) or fade.get("measure") not in (
                "axis",
                "x",
                "y",
                "min",
                "max",
            ):
                raise ValueError("invalid optical-size fade")
        dependencies[i] = []
        if node["op"] in ("link", "center"):
            reference = node["ref" if node["op"] == "link" else "other"]
            if frozenset((i, reference)) not in pairs:
                raise ValueError("hint link is not an observed stem")
            if node["op"] == "link" and nodes[reference] is None:
                raise ValueError("hint link requires a touched anchor")
            if node["op"] == "link":
                dependencies[i] = [reference]
        elif node["op"] == "relative":
            reference = node["ref"]
            if frozenset((i, reference)) not in pairs | counters:
                raise ValueError("relative link is not an observed stroke or counter")
            dependencies[i] = [reference]
        elif node["op"] == "interpolate":
            left, right = node["refs"]
            if not (
                0 <= left < len(nodes) and 0 <= right < len(nodes) and left != right
            ):
                raise ValueError("invalid interpolation anchors")
            positions = [g["value"] for g in feature["groups"]]
            if (
                not min(positions[left], positions[right])
                < positions[i]
                < max(positions[left], positions[right])
            ):
                raise ValueError("interpolation requires bracketing anchors")
            dependencies[i] = [left, right]
        if any(nodes[j] is None for j in dependencies[i]):
            raise ValueError("hint relation requires touched anchors")
        if any(nodes[j]["op"] == "adjust" for j in dependencies[i]):
            raise ValueError("primary hint cannot depend on a later adjustment")
    result = []
    while pending:
        ready = [i for i in sorted(pending) if not (set(dependencies[i]) & pending)]
        if not ready:
            raise ValueError("cyclic hint references")
        result.extend(ready)
        pending.difference_update(ready)
    return result


def regime_matches(regime, axis, projected):
    measure = regime.get("measure", "axis")
    if measure not in ("axis", "x", "y", "min", "max"):
        raise ValueError("unknown hint regime measure")
    size = {
        "axis": projected[axis],
        "x": projected[0],
        "y": projected[1],
        "min": min(projected),
        "max": max(projected),
    }[measure]
    return size <= regime["limit"]


def build(state, characters=None, witnesses=False):
    shapes = state["shapes"]
    characters = sorted(shapes if characters is None else characters)
    if not characters or any(not 33 <= ord(c) <= 126 for c in characters):
        raise ValueError("joint reconstruction requires visible ASCII glyphs")
    params = state["parameters"]
    if not 0 <= params["cutin"] <= 64 or not 1 <= params["limit"] <= 4096:
        raise ValueError("invalid shared hint limits")
    cvt = []

    def scaled(n):
        n = round(n)
        if not -32768 <= n <= 32767:
            raise ValueError("CVT coordinate out of range")
        if n not in cvt:
            cvt.append(n)
        return font_probe.push(cvt.index(n)) + bytes([0x45])

    def bias(expr, amount, node=None, axis=0):
        if not amount:
            return expr
        fade = node.get("fade") if node else None
        adjustment = font_probe.push(amount)
        if fade:
            measure = fade["measure"]

            def size(a):
                return bytes([0x01 if a == 0 else 0x00, 0x4B])

            measured = (
                size(0) + size(1) + bytes([0x8C if measure == "min" else 0x8B])
                if measure in ("min", "max")
                else size(axis if measure == "axis" else 0 if measure == "x" else 1)
            )
            # F26Dot6 correction times clamp((end - ppem)/(end - 10), 0, 1).
            # MPPEM is an integer; convert the factor to F26Dot6 before MUL.
            # Restore the movement axis after measuring the optical size.
            adjustment += (
                font_probe.push(fade["end"])
                + measured
                + bytes([0x61])
                + font_probe.push(0)
                + bytes([0x8B])
                + font_probe.push(fade["end"] - 10)
                + bytes([0x8C])
                + font_probe.push(4096)
                + bytes([0x63])
                + font_probe.push((fade["end"] - 10) * 64)
                + bytes([0x62, 0x63, 0x01 if axis == 0 else 0x00])
            )
        return expr + adjustment + bytes([0x60])

    def rounded(n, mode, phase=0, node=None, axis=0):
        expr = (
            bytes([ROUNDS[mode]])
            + bias(scaled(abs(n)), phase, node, axis)
            + bytes([0x68])
        )
        return expr + bytes([0x65]) if n < 0 else expr

    def width(n, axis, node):
        expr = scaled(abs(n))
        index = node.get("stem")
        if index is not None:
            if not 0 <= index < len(params["widths"][axis]):
                raise ValueError("invalid shared stem index")
            shared = params["widths"][axis][index]
            expr += (
                bytes([0x20])
                + scaled(shared)
                + bytes([0x61, 0x64])
                + font_probe.push(params["cutin"])
                + bytes([0x50, 0x58, 0x21])
                + scaled(shared)
                + bytes([0x59])
            )
        return (
            bias(expr, node.get("phase", 0), node, axis)
            + bytes([ROUNDS[node["round"]], 0x68])
            + font_probe.push(64)
            + bytes([0x8B])
        )

    def condition(regime, axis):
        measure = regime.get("measure", "axis")
        regime_matches(regime, axis, (1, 1))

        def test(a):
            return (
                bytes([0x01 if a == 0 else 0x00, 0x4B])
                + font_probe.push(regime["limit"])
                + bytes([0x51])
            )

        if measure in ("min", "max"):
            return test(0) + test(1) + bytes([0x5B if measure == "min" else 0x5A])
        return test(axis if measure == "axis" else 0 if measure == "x" else 1)

    def compile_axis(char, axis, nodes):
        program = b""
        # Keep the original emission order byte-identical when no extensions
        # are requested, so all previously frozen fonts remain reproducible.
        feature = state["graph"][char][axis]
        groups = feature["groups"]
        positions = [value(shapes[char], group, axis) for group in groups]
        ordered_nodes = ordered(nodes, feature)
        order = [i for i in ordered_nodes if nodes[i]["op"] != "adjust"]
        for i in order:
            node = nodes[i]
            position = positions[i]
            mode = node["round"]
            phase = node.get("phase", 0)
            if node["op"] == "interpolate":
                left, right = node["refs"]
                if positions[left] == positions[right]:
                    raise ValueError("coincident interpolation anchors")
                # OpenType IP, SRP1 and SRP2: preserve the original relative
                # position between two fitted anchors; IUP then moves the
                # remaining contour points. No per-size coordinate table.
                program += (
                    bytes([0x01 if axis == 0 else 0x00])
                    + font_probe.push(groups[left]["points"][0])
                    + bytes([0x11])
                    + font_probe.push(groups[right]["points"][0])
                    + bytes([0x12])
                )
                for point in groups[i]["points"]:
                    program += font_probe.push(point) + bytes([0x39])
                    if mode != "none":
                        program += (
                            font_probe.push(point)
                            + font_probe.push(point)
                            + bytes([0x46, ROUNDS[mode], 0x68, 0x48])
                        )
                    if node.get("shift", 0):
                        program += (
                            font_probe.push(point)
                            + bias(
                                font_probe.push(point) + bytes([0x46]),
                                node["shift"],
                                node,
                                axis,
                            )
                            + bytes([0x48])
                        )
                continue
            if node["op"] == "anchor":
                expr = rounded(
                    position + node.get("design", 0), mode, phase, node, axis
                )
            elif node["op"] == "zone":
                zone = node["zone"]
                if axis != 1 or not 0 <= zone < len(params["zones"]):
                    raise ValueError("invalid height zone")
                expr = (
                    rounded(params["zones"][zone], mode, phase, node, axis)
                    + rounded(position - state["zone_origins"][zone], mode)
                    + bytes([0x60])
                )
            elif node["op"] == "center":
                other = node["other"]
                expr = (
                    scaled((position + positions[other]) / 2)
                    + width(position - positions[other], axis, node)
                    + font_probe.push(128)
                    + bytes(
                        [
                            0x62,
                            0x61 if position < positions[other] else 0x60,
                            ROUNDS[mode],
                            0x68,
                        ]
                    )
                )
            elif node["op"] == "relative":
                reference = node["ref"]
                distance = position - positions[reference]
                expr = font_probe.push(groups[reference]["points"][0]) + bytes([0x46])
                expr += (
                    scaled(abs(distance))
                    if mode == "none"
                    else rounded(abs(distance), mode, phase, node, axis)
                )
                # Relative counter distances are not forced to an ink-stem
                # CVT or one-pixel width. Optional minimum preserves a gap.
                if node.get("minimum", False):
                    expr += font_probe.push(64) + bytes([0x8B])
                expr += bytes([0x61 if distance < 0 else 0x60])
            else:
                reference = node["ref"]
                expr = (
                    font_probe.push(groups[reference]["points"][0])
                    + bytes([0x46])
                    + width(position - positions[reference], axis, node)
                    + bytes([0x61 if position < positions[reference] else 0x60])
                )
            # F26Dot6 bias before rounding changes a threshold; shift after
            # rounding preserves a fractional edge position. Each parameter
            # applies over a whole program range, never one requested size.
            expr = bias(expr, node.get("shift", 0), node, axis)
            for point in groups[i]["points"]:
                program += (
                    bytes([0x01 if axis == 0 else 0x00])
                    + font_probe.push(point)
                    + expr
                    + bytes([0x48])
                )
        if order:
            program += bytes([0x31 if axis == 0 else 0x30])
        adjustments = [i for i in ordered_nodes if nodes[i]["op"] == "adjust"]
        for i in adjustments:
            node = nodes[i]
            moved = bytes([0x01 if axis == 0 else 0x00])
            for point in groups[i]["points"]:
                moved += (
                    font_probe.push(point)
                    + bias(
                        font_probe.push(point) + bytes([0x46]),
                        node["shift"],
                        node,
                        axis,
                    )
                    + bytes([0x48])
                )
            if node.get("fade"):
                moved = (
                    condition(
                        dict(
                            limit=node["fade"]["end"] - 1,
                            measure=node["fade"]["measure"],
                        ),
                        axis,
                    )
                    + bytes([0x58])
                    + moved
                    + bytes([0x59])
                )
            program += moved
        if adjustments:
            # Keep fitted stems/zones touched; IUP propagates only the new
            # shoulder correction between these fixed anchors.
            program += bytes([0x01 if axis == 0 else 0x00, 0x31 if axis == 0 else 0x30])
        return program

    programs = {}
    for char in characters:
        program = b""
        independent = char in state.get("independent_axes", [])
        regimes = state.get("regimes", {}).get(char, [None, None])
        optical = state.get("optical_programs", {}).get(char)
        if optical and (
            optical.get("limit") not in (10, 12, 16, 24, 32)
            or optical["limit"] > params["limit"]
            or len(optical.get("nodes", [])) != 2
            or len(optical.get("measures", ["max", "max"])) != 2
            or any(
                m not in ("axis", "x", "y", "min", "max")
                for m in optical.get("measures", [])
            )
        ):
            raise ValueError("invalid small-square optical program")
        if len(regimes) != 2:
            raise ValueError("invalid axis regimes")
        for axis, nodes in enumerate(state["programs"][char]):
            main = compile_axis(char, axis, nodes)
            regime = regimes[axis]
            if regime is None:
                axis_program = main
            else:
                if not 1 <= regime["limit"] <= params["limit"]:
                    raise ValueError("invalid projected-ppem regime")
                small = compile_axis(char, axis, regime["nodes"])
                smaller = regime.get("smaller")
                if smaller:
                    if not (
                        1 <= smaller["limit"] <= regime["limit"]
                        and (
                            smaller["limit"] < regime["limit"]
                            or smaller.get("measure", "axis")
                            != regime.get("measure", "axis")
                        )
                    ):
                        raise ValueError(
                            "smallest regime must be below its parent or refine its measure"
                        )
                    small = (
                        condition(smaller, axis)
                        + bytes([0x58])
                        + compile_axis(char, axis, smaller["nodes"])
                        + bytes([0x1B])
                        + small
                        + bytes([0x59])
                    )
                # MPPEM measures the projection direction. One coarse branch per
                # axis allows fine-stroke behavior without changing medium sizes.
                axis_program = (
                    condition(regime, axis)
                    + bytes([0x58])
                    + small
                    + bytes([0x1B])
                    + main
                    + bytes([0x59])
                )
            if optical and optical["nodes"][axis] is not None:
                axis_program = (
                    condition(
                        dict(
                            limit=optical["limit"],
                            measure=optical.get("measures", ["max", "max"])[axis],
                        ),
                        axis,
                    )
                    + bytes([0x58])
                    + compile_axis(char, axis, optical["nodes"][axis])
                    + bytes([0x1B])
                    + axis_program
                    + bytes([0x59])
                )
            if independent and axis_program:
                axis_program = (
                    bytes([0x01 if axis == 0 else 0x00, 0x4B])
                    + font_probe.push(params["limit"])
                    + bytes([0x51, 0x58])
                    + axis_program
                    + bytes([0x59])
                )
            program += axis_program
        if program and not independent:
            limit = font_probe.push(params["limit"])
            program = (
                bytes([0x01, 0x4B])
                + limit
                + bytes([0x51, 0x00, 0x4B])
                + limit
                + bytes([0x51, 0x5A, 0x58])
                + program
                + bytes([0x59])
            )
        programs[char] = program
    items = [
        dict(
            name=f"joint-{code}",
            codepoint=code,
            contours=shapes.get(chr(code), []) if chr(code) in characters else [],
            instructions=programs.get(chr(code), b"").hex(),
            advance=state.get("advances", {}).get(chr(code), 2048),
            family="joint",
        )
        for code in range(32, max(map(ord, characters)) + 1)
    ]
    if witnesses:
        items = font_probe.add_witnesses(items, characters)
    return font_probe.font(
        items, {b"cvt ": bytearray(struct.pack(">" + str(len(cvt)) + "h", *cvt))}
    )


def complexity(program):
    return sum(
        1 + (node["op"] in ("link", "center"))
        for axis in program
        for node in axis
        if node
    )


def neighbors(state, char):
    """Single-feature edits and paired stem moves; no arbitrary bytecode search."""
    graph = state["graph"][char]
    program = state["programs"][char]
    for axis, feature in enumerate(graph):
        for i, group in enumerate(feature["groups"]):
            choices = [None] + [dict(op="anchor", round=mode) for mode in ROUNDS]
            if axis == 1:
                choices += [
                    dict(op="zone", zone=z, round="grid")
                    for z, v in enumerate(state["zone_origins"])
                    if abs(v - group["value"]) <= 64
                ]
            for node in choices:
                if node == program[axis][i]:
                    continue
                candidate = deepcopy(program)
                candidate[axis][i] = node
                try:
                    ordered(candidate[axis], feature)
                except ValueError:
                    continue
                yield candidate
        for pair in feature["stems"]:
            low, high = pair["low"], pair["high"]
            for anchor, other in ((low, high), (high, low)):
                for mode in ROUNDS:
                    for stem in (
                        None,
                        shared_index(state["parameters"], axis, pair["width"]),
                    ):
                        candidate = deepcopy(program)
                        candidate[axis][anchor] = dict(op="anchor", round=mode)
                        candidate[axis][other] = dict(
                            op="link", ref=anchor, stem=stem, round="grid"
                        )
                        yield candidate
                candidate = deepcopy(program)
                stem = shared_index(state["parameters"], axis, pair["width"])
                candidate[axis][anchor] = dict(
                    op="center", other=other, stem=stem, round="grid"
                )
                candidate[axis][other] = dict(
                    op="link", ref=anchor, stem=stem, round="grid"
                )
                yield candidate
