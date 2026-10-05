"""Geometry-derived optical corrections perpendicular to diagonal ink strokes.

SFVTL[1] obtains the perpendicular from scaled outline points, so anisotropic
sizes do not use an angle fixed in design space. SHPIX then moves both edges
symmetrically. OpenType TrueType instructions, SFVTL and SHPIX:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
This is one bounded optical stroke parameter over a coarse range, not DELTAP.
"""

from itertools import combinations
import math

import font_probe
from reconstruct_geometry import flatten, winding


def strokes(shape):
    points = [p for contour in shape for p in contour]
    loops = [flatten(contour) for contour in shape]
    edges = []
    offset = 0
    for contour in shape:
        for i, a in enumerate(contour):
            j = (i + 1) % len(contour)
            b = contour[j]
            dx, dy = b[0] - a[0], b[1] - a[1]
            length = math.hypot(dx, dy)
            if a[2] and b[2] and length >= 256 and min(abs(dx), abs(dy)) >= 64:
                members = sorted(
                    {
                        offset + k
                        for k, p in enumerate(contour)
                        if min(math.dist(p[:2], a[:2]), math.dist(p[:2], b[:2])) <= 8
                    }
                )
                edges.append(
                    dict(
                        line=[offset + i, offset + j],
                        points=members,
                        unit=(dx / length, dy / length),
                        length=length,
                    )
                )
        offset += len(contour)
    choices = []
    for i, j in combinations(range(len(edges)), 2):
        a, b = edges[i], edges[j]
        if sum(x * y for x, y in zip(a["unit"], b["unit"], strict=True)) > -0.98:
            continue
        start = points[a["line"][0]]
        u, v = a["unit"]
        normal = (-v, u)
        positions = [
            sum((points[k][axis] - start[axis]) * a["unit"][axis] for axis in (0, 1))
            for k in b["line"]
        ]
        low, high = max(0, min(positions)), min(a["length"], max(positions))
        if high - low < 128:
            continue
        distances = [
            sum((points[k][axis] - start[axis]) * normal[axis] for axis in (0, 1))
            for k in b["line"]
        ]
        distance = sum(distances) / 2
        if not 32 <= abs(distance) <= 384 or abs(distances[0] - distances[1]) > 32:
            continue
        if not all(
            winding(
                (
                    start[0] + u * t + normal[0] * distance * s,
                    start[1] + v * t + normal[1] * distance * s,
                ),
                loops,
            )
            for t in (low + (high - low) * f for f in (0.25, 0.5, 0.75))
            for s in (0.25, 0.5, 0.75)
        ):
            continue
        choices.append((-(high - low), abs(distance), i, j, 1 if distance > 0 else -1))
    result, used = [], set()
    for _, width, i, j, side in sorted(choices):
        if i in used or j in used:
            continue
        used.update((i, j))
        result.append(
            dict(
                line=edges[i]["line"],
                edges=[edges[i]["points"], edges[j]["points"]],
                side=side,
                width=width,
            )
        )
        if len(result) == 12:
            break
    return result


def compile(shape, config):
    if not isinstance(config.get("shift"), int) or not -32 <= config["shift"] <= 32:
        raise ValueError("diagonal stroke shift exceeds half a pixel per edge")
    found = strokes(shape)
    if not found:
        raise ValueError("diagonal correction requires an observed ink corridor")
    if not config["shift"]:
        return b""
    program = b""
    for stroke in found:
        a, b = stroke["line"]
        # SFVTL uses the glyph zone and the current scaled line; SHPIX does not
        # depend on the projection vector. Set the vector once before moving
        # either edge, then restore an axis before the next program condition.
        # p1 is the top operand (a), p2 the next (b): a -> b, then CCW.
        program += font_probe.push(b) + font_probe.push(a) + bytes([0x09])
        for edge, sign in zip(stroke["edges"], (1, -1), strict=True):
            for point in edge:
                program += (
                    font_probe.push(point)
                    + font_probe.push(sign * stroke["side"] * config["shift"])
                    + bytes([0x38])
                )
    return program + bytes([0x01])
