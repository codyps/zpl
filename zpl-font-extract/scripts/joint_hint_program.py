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
        ) or not (
            node.get("round") in ROUNDS
            or (
                node.get("op") in ("relative", "interpolate")
                and node.get("round") == "none"
            )
        ):
            raise ValueError("invalid hint operation")
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
    result = []
    while pending:
        ready = [i for i in sorted(pending) if not (set(dependencies[i]) & pending)]
        if not ready:
            raise ValueError("cyclic hint references")
        result.extend(ready)
        pending.difference_update(ready)
    return result


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

    def rounded(n, mode):
        expr = bytes([ROUNDS[mode]]) + scaled(abs(n)) + bytes([0x68])
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
            expr
            + bytes([ROUNDS[node["round"]], 0x68])
            + font_probe.push(64)
            + bytes([0x8B])
        )

    def compile_axis(char, axis, nodes):
        program = b""
        # Keep the original emission order byte-identical when no extensions
        # are requested, so all previously frozen fonts remain reproducible.
        feature = state["graph"][char][axis]
        groups = feature["groups"]
        positions = [value(shapes[char], group, axis) for group in groups]
        order = ordered(nodes, feature)
        for i in order:
            node = nodes[i]
            position = positions[i]
            mode = node["round"]
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
                continue
            if node["op"] == "anchor":
                expr = rounded(position, mode)
            elif node["op"] == "zone":
                zone = node["zone"]
                if axis != 1 or not 0 <= zone < len(params["zones"]):
                    raise ValueError("invalid height zone")
                expr = (
                    rounded(params["zones"][zone], mode)
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
                    else rounded(abs(distance), mode)
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
            for point in groups[i]["points"]:
                program += (
                    bytes([0x01 if axis == 0 else 0x00])
                    + font_probe.push(point)
                    + expr
                    + bytes([0x48])
                )
        if order:
            program += bytes([0x31 if axis == 0 else 0x30])
        return program

    programs = {}
    for char in characters:
        program = b""
        regimes = state.get("regimes", {}).get(char, [None, None])
        if len(regimes) != 2:
            raise ValueError("invalid axis regimes")
        for axis, nodes in enumerate(state["programs"][char]):
            main = compile_axis(char, axis, nodes)
            regime = regimes[axis]
            if regime is None:
                program += main
                continue
            if not 1 <= regime["limit"] <= params["limit"]:
                raise ValueError("invalid projected-ppem regime")
            small = compile_axis(char, axis, regime["nodes"])
            # MPPEM measures the projection direction. One coarse branch per
            # axis allows fine-stroke behavior without changing medium sizes.
            program += (
                bytes([0x01 if axis == 0 else 0x00, 0x4B])
                + font_probe.push(regime["limit"])
                + bytes([0x51, 0x58])
                + small
                + bytes([0x1B])
                + main
                + bytes([0x59])
            )
        if program:
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
            advance=2048,
            family="joint",
        )
        for code in range(32, max(map(ord, characters)) + 1)
    ]
    if witnesses:
        if "!" in characters or '"' in characters or len(items) < 3:
            raise ValueError("printer witnesses require unused ! and quotation mark")
        items[1] = font_probe.glyphs()[1]
        items[2] = dict(
            name="instruction-witness",
            codepoint=34,
            contours=[font_probe.rect(0, 0, 1024, 256)],
            instructions=font_probe.set_right(font_probe.push(13 * 64)).hex(),
            advance=1024,
        )
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
