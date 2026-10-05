"""Geometry-constrained hint construction, without per-glyph or per-size rules.

Only observed opposing ink edges can form stems. Shared widths and height zones
are robust clusters of those features. Programs use RCVT, ROUND, GC, SCFS and
IUP as defined in the OpenType instruction reference (individual opcode sections):
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
"""

import statistics
import struct

import font_probe
from reconstruct_geometry import flatten, winding


def clusters(values, tolerance):
    result = []
    for value in sorted(values):
        if result and value - result[-1][0] <= tolerance:
            result[-1].append(value)
        else:
            result.append([value])
    return result


def features(shape, axis):
    points = [p for contour in shape for p in contour]
    selected = set()
    offset = 0
    for contour in shape:
        on = [p for p in contour if p[2]]
        bounds = (min(p[axis] for p in on), max(p[axis] for p in on))
        for i, p in enumerate(contour):
            q = contour[(i + 1) % len(contour)]
            if p[2] and p[axis] in bounds:
                selected.add(offset + i)
            if (
                p[2]
                and q[2]
                and p[axis] == q[axis]
                and abs(p[1 - axis] - q[1 - axis]) >= 64
            ):
                selected.update((offset + i, offset + (i + 1) % len(contour)))
        offset += len(contour)
    groups = []
    for cluster in clusters({points[i][axis] for i in selected}, 4):
        members = sorted(i for i in selected if points[i][axis] in cluster)
        groups.append(
            dict(
                value=round(statistics.median(cluster)),
                points=members,
                span=[
                    min(points[i][1 - axis] for i in members),
                    max(points[i][1 - axis] for i in members),
                ],
            )
        )
    loops = [flatten(c) for c in shape]
    candidates = []
    for i, a in enumerate(groups):
        for j in range(i + 1, len(groups)):
            b = groups[j]
            width = b["value"] - a["value"]
            if not 32 <= width <= 512:
                continue
            lo = max(a["span"][0], b["span"][0])
            hi = min(a["span"][1], b["span"][1])
            if lo > hi + 32:
                continue
            center = (a["value"] + b["value"]) / 2
            samples = (
                [(lo + hi) / 2]
                if lo >= hi
                else [lo + (hi - lo) * t for t in (0.25, 0.5, 0.75)]
            )
            if all(
                winding((center, s) if axis == 0 else (s, center), loops)
                for s in samples
            ):
                candidates.append(
                    dict(low=i, high=j, width=width, support=max(1, hi - lo))
                )
    # Non-overlapping narrow pairs keep a counter from being mistaken for ink.
    paired, used = [], set()
    for pair in sorted(candidates, key=lambda p: (p["width"], -p["support"], p["low"])):
        if not ({pair["low"], pair["high"]} & used):
            paired.append(pair)
            used.update((pair["low"], pair["high"]))
    return dict(groups=groups, stems=paired)


def infer(shapes):
    graph = {c: [features(shape, a) for a in (0, 1)] for c, shape in shapes.items()}
    shared = []
    for axis in (0, 1):
        values = [p["width"] for g in graph.values() for p in g[axis]["stems"]]
        shared.append(
            [round(statistics.median(v)) for v in clusters(values, 32) if len(v) >= 2]
        )
    heights = [g["value"] for axes in graph.values() for g in axes[1]["groups"]]
    zones = [round(statistics.median(v)) for v in clusters(heights, 48) if len(v) >= 2]
    return dict(graph=graph, shared_widths=shared, zones=zones)


POLICIES = ("none", "grid", "stems-low", "stems-high", "stems-center", "zones")


def build(shapes, inferred, policies, cutin=24, witnesses=False):
    cvt = []

    def scaled(value):
        value = round(value)
        if value not in cvt:
            cvt.append(value)
        return font_probe.push(cvt.index(value)) + bytes([0x45])

    def rounded(value):
        expr = scaled(abs(value)) + bytes([0x68])
        return expr + bytes([0x65]) if value < 0 else expr

    programs = {}
    for char, shape in shapes.items():
        program = b""
        for axis, policy in enumerate(policies[char]):
            if policy not in POLICIES:
                raise ValueError("unknown hint policy")
            if policy == "none":
                continue
            feature = inferred["graph"][char][axis]
            groups = feature["groups"]
            expressions = {}
            if policy in ("grid", "zones"):
                for i, group in enumerate(groups):
                    value = group["value"]
                    zone = (
                        min(inferred["zones"], key=lambda z: abs(z - value))
                        if axis == 1 and inferred["zones"]
                        else value
                    )
                    if policy == "zones" and abs(zone - value) <= 48:
                        expressions[i] = (
                            rounded(zone) + rounded(value - zone) + bytes([0x60])
                        )
                    else:
                        expressions[i] = rounded(value)
            else:
                # An observed stem is moved as a unit. There are no arbitrary
                # links across counters, no per-size deltas, and no cyclic graph.
                for pair in feature["stems"]:
                    low, high, width = pair["low"], pair["high"], pair["width"]
                    expr = scaled(width)
                    widths = inferred["shared_widths"][axis]
                    shared = (
                        min(widths, key=lambda v: abs(v - width)) if widths else width
                    )
                    if abs(shared - width) <= 32:
                        expr += (
                            bytes([0x20])
                            + scaled(shared)
                            + bytes([0x61, 0x64])
                            + font_probe.push(cutin)
                            + bytes([0x50, 0x58, 0x21])
                            + scaled(shared)
                            + bytes([0x59])
                        )
                    expr += bytes([0x68]) + font_probe.push(64) + bytes([0x8B])
                    anchor, other = (
                        (high, low) if policy == "stems-high" else (low, high)
                    )
                    if policy == "stems-center":
                        anchor_expr = (
                            scaled((groups[low]["value"] + groups[high]["value"]) / 2)
                            + expr
                            + font_probe.push(128)
                            + bytes([0x62, 0x61, 0x68])
                        )
                    else:
                        anchor_expr = rounded(groups[anchor]["value"])
                    expressions[anchor] = anchor_expr
                    # Inline anchor expression avoids dependency ordering and
                    # reuses exactly the same rounded stem origin for both edges.
                    expressions[other] = (
                        anchor_expr + expr + bytes([0x61 if other == low else 0x60])
                    )
            for i, expr in sorted(expressions.items()):
                for point in groups[i]["points"]:
                    program += (
                        bytes([0x01 if axis == 0 else 0x00])
                        + font_probe.push(point)
                        + expr
                        + bytes([0x48])
                    )
            program += bytes([0x31 if axis == 0 else 0x30])
        limit = inferred.get("hint_ppem_limit")
        if limit is not None and program:
            # A single font-wide operating range, derived from the gap between
            # small training sizes and large geometry measurements. Outside it,
            # preserve the fitted outline instead of imposing small-size snaps.
            guard = (
                bytes([0x01, 0x4B])
                + font_probe.push(limit)
                + bytes([0x51])
                + bytes([0x00, 0x4B])
                + font_probe.push(limit)
                + bytes([0x51, 0x5A, 0x58])
            )
            program = guard + program + bytes([0x59])
        programs[char] = program
    if not shapes or not all(32 <= ord(c) <= 126 for c in shapes):
        raise ValueError("reconstruction currently supports nonempty ASCII glyph sets")
    items = [
        dict(
            name=f"reconstructed-{code}",
            codepoint=code,
            contours=shapes.get(chr(code), []),
            instructions=programs.get(chr(code), b"").hex(),
            advance=2048,
            family="reconstruction",
        )
        for code in range(32, max(map(ord, shapes)) + 1)
    ]
    if witnesses:
        if "!" in shapes or '"' in shapes or len(items) < 3:
            raise ValueError(
                "printer witnesses require unused ASCII ! and quotation mark slots"
            )
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
