"""Original quadratic outline reconstruction from oriented pixel-cell contours.

Endpoints, corners and extrema are constrained; the remaining quadratic control
point minimizes squared distances after monotone nearest-parameter refinement.
This is an original implementation, not a port of a tracing library. The emitted
on/off-curve representation follows OpenType 'glyf', Simple Glyph Description:
https://learn.microsoft.com/en-us/typography/opentype/spec/glyf
"""

import math

from analyze_font0_outline import trace


class FitRejected(ValueError):
    """An expected geometry constraint failure, not an implementation error."""


def distance(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def lerp(a, b, t):
    return tuple(a[k] + t * (b[k] - a[k]) for k in (0, 1))


def quadratic(a, b, c, t):
    return lerp(lerp(a, b, t), lerp(b, c, t), t)


def line_error(p, a, b):
    delta = [b[k] - a[k] for k in (0, 1)]
    den = sum(d * d for d in delta)
    t = (
        max(0, min(1, sum((p[k] - a[k]) * delta[k] for k in (0, 1)) / den))
        if den
        else 0
    )
    return distance(p, lerp(a, b, t))


def fit_arc(points, tolerance, depth=0):
    """Return explicit endpoint/control sequences; split at worst residual."""
    if depth > 40:
        raise FitRejected("curve subdivision budget exceeded")
    a, c = points[0], points[-1]
    if max((line_error(p, a, c) for p in points), default=0) <= tolerance:
        return [(*a, True), (*c, True)]
    lengths = [0.0]
    for p, q in zip(points, points[1:]):
        lengths.append(lengths[-1] + distance(p, q))
    ts = [s / lengths[-1] for s in lengths]
    for _ in range(4):
        weights = [2 * t * (1 - t) for t in ts]
        denominator = sum(w * w for w in weights)
        if denominator == 0:
            break
        b = tuple(
            sum(
                w * (p[k] - (1 - t) ** 2 * a[k] - t * t * c[k])
                for p, t, w in zip(points, ts, weights)
            )
            / denominator
            for k in (0, 1)
        )
        updated = [0.0]
        for p, t in zip(points[1:-1], ts[1:-1]):
            q = quadratic(a, b, c, t)
            derivative = [
                2 * ((1 - t) * (b[k] - a[k]) + t * (c[k] - b[k])) for k in (0, 1)
            ]
            second = [2 * (c[k] - 2 * b[k] + a[k]) for k in (0, 1)]
            den = sum(derivative[k] ** 2 + (q[k] - p[k]) * second[k] for k in (0, 1))
            step = (
                sum((q[k] - p[k]) * derivative[k] for k in (0, 1)) / den
                if den > 0
                else 0
            )
            updated.append(max(updated[-1], min(1, t - step)))
        ts = updated + [1.0]
    errors = [distance(p, quadratic(a, b, c, t)) for p, t in zip(points, ts)]
    if max(errors) <= tolerance:
        return [(*a, True), (*b, False), (*c, True)]
    cut = max(range(1, len(points) - 1), key=lambda i: errors[i])
    return fit_arc(points[: cut + 1], tolerance, depth + 1)[:-1] + fit_arc(
        points[cut:], tolerance, depth + 1
    )


def anchors(loop):
    """Lock extrema plateaus, long straight runs and sustained sharp turns."""
    n = len(loop)
    result = {0}
    for axis in (0, 1):
        for value in (min(p[axis] for p in loop), max(p[axis] for p in loop)):
            selected = [i for i, p in enumerate(loop) if p[axis] == value]
            # Each contiguous plateau needs endpoints; this preserves flat stems.
            for i in selected:
                if loop[(i - 1) % n][axis] != value or loop[(i + 1) % n][axis] != value:
                    result.add(i)
            if len(selected) == 1:
                result.add(selected[0])
    # Scan maximal runs directly, avoiding a corner on every staircase pixel.
    directions = [
        (loop[(i + 1) % n][0] - p[0], loop[(i + 1) % n][1] - p[1])
        for i, p in enumerate(loop)
    ]
    for i in range(n):
        if directions[i] == directions[(i - 1) % n]:
            continue
        length = 1
        while length < n and directions[(i + length) % n] == directions[i]:
            length += 1
        if length >= 12:
            result.update((i, (i + length) % n))
    span = min(8, max(1, n // 8))
    turns = []
    for i, p in enumerate(loop):
        a, b = loop[(i - span) % n], loop[(i + span) % n]
        u, v = (p[0] - a[0], p[1] - a[1]), (b[0] - p[0], b[1] - p[1])
        den = math.hypot(*u) * math.hypot(*v)
        turns.append((u[0] * v[0] + u[1] * v[1]) / den if den else 1)
    for i in sorted(range(n), key=lambda i: (turns[i], i)):
        if turns[i] < 0.6 and all(
            min((i - j) % n, (j - i) % n) >= span for j in result
        ):
            result.add(i)
    if len(result) < 2:
        result.add(max(range(n), key=lambda i: distance(loop[i], loop[0])))
    return sorted(result)


def split_extrema(a, b, c):
    a, b, c = a[:2], b[:2], c[:2]
    ts = []
    for k in (0, 1):
        den = a[k] - 2 * b[k] + c[k]
        t = (a[k] - b[k]) / den if den else 0
        if 1e-8 < t < 1 - 1e-8:
            ts.append(t)
    result = [(*a, True)]
    previous = 0
    for t in sorted(set(ts)):
        fraction = (t - previous) / (1 - previous)
        ab, bc = lerp(a, b, fraction), lerp(b, c, fraction)
        middle = lerp(ab, bc, fraction)
        result.extend([(*ab, False), (*middle, True)])
        a, b, previous = middle, bc, t
    return result + [(*b, False), (*c, True)]


def fit_shape(pixels, tolerance, scale):
    result = []
    for loop in trace(pixels):
        stops = anchors(loop)
        outline = []
        for index, start in enumerate(stops):
            end = stops[(index + 1) % len(stops)]
            points = (
                loop[start : end + 1] if end > start else loop[start:] + loop[: end + 1]
            )
            arc = fit_arc(points, tolerance)
            i = 0
            while i < len(arc) - 1:
                if arc[i + 1][2]:
                    outline.append(arc[i])
                    i += 1
                else:
                    outline.extend(split_extrema(arc[i], arc[i + 1], arc[i + 2])[:-1])
                    i += 2
        quantized = [(round(x * scale), round(-y * scale), on) for x, y, on in outline]
        result.append(quantized)
    if not topology_matches(
        [[(*p, True) for p in loop] for loop in trace(pixels)],
        [[(x / scale, -y / scale, on) for x, y, on in c] for c in result],
    ):
        raise FitRejected("quadratic fit changes contour topology")
    return result


def flatten(contour, tolerance=0.5):
    result = []

    def curve(a, b, c):
        if line_error(b, a, c) <= tolerance:
            result.append(a[:2])
        else:
            ab, bc = lerp(a, b, 0.5), lerp(b, c, 0.5)
            middle = lerp(ab, bc, 0.5)
            curve(a, ab, middle)
            curve(middle, bc, c)

    i = 0
    while i < len(contour):
        a, b = contour[i], contour[(i + 1) % len(contour)]
        if not a[2]:
            raise ValueError("expected explicit on-curve endpoint")
        if b[2]:
            result.append(a[:2])
            i += 1
        else:
            c = contour[(i + 2) % len(contour)]
            if not c[2]:
                raise ValueError("expected explicit quadratic endpoint")
            curve(a, b, c)
            i += 2
    return result


def area(loop):
    return sum(a[0] * b[1] - a[1] * b[0] for a, b in zip(loop, loop[1:] + loop[:1])) / 2


def winding(point, loops):
    x, y = point
    result = 0
    for loop in loops:
        for a, b in zip(loop, loop[1:] + loop[:1]):
            cross = (b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1])
            if a[1] <= y < b[1] and cross > 0:
                result += 1
            if b[1] <= y < a[1] and cross < 0:
                result -= 1
    return result


def topology_matches(before, after):
    """Check winding, containment and proper segment crossings after fitting."""
    old, new = [list(map(flatten, shape)) for shape in (before, after)]
    if len(old) != len(new) or any(area(a) * area(b) <= 0 for a, b in zip(old, new)):
        return False
    for i in range(len(old)):
        for j in range(len(old)):
            if i != j and bool(winding(old[i][0], [old[j]])) != bool(
                winding(new[i][0], [new[j]])
            ):
                return False
    segments = [(a, b) for loop in new for a, b in zip(loop, loop[1:] + loop[:1])]

    def cross(a, b, p):
        return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])

    for i, (a, b) in enumerate(segments):
        for c, d in segments[i + 1 :]:
            if max(a[0], b[0]) < min(c[0], d[0]) or max(c[0], d[0]) < min(a[0], b[0]):
                continue
            if (
                cross(a, b, c) * cross(a, b, d) < 0
                and cross(c, d, a) * cross(c, d, b) < 0
            ):
                return False
    return True
