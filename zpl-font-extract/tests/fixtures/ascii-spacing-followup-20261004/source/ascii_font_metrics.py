"""Recover design advances from repeated native-origin sentinel measurements.

Extraction follows the same visible-terminal check as zpl-font-extract/src/lib.rs.
The layout scaling equation is independently measured for uploaded TTFs and
implemented by zpl::truetype::Environment::ppem(layout=true). No per-size advances
are emitted. Incompatible observations remain explicit fitting residuals.
"""

import math

from reconstruct_font import require


def layout_ppem(dots):
    return ((max(dots, 10) * 1152 + 202) // 203) * 184958 / (16 * 65536)


def layout_advance(units, dots):
    return math.floor(units * layout_ppem(dots) / 2048 + 0.5)


def selected(reference, probe):
    tx, ty, w, h = probe["tile"]
    ax, ay = probe["anchor"]
    points = {
        (x - tx - ax, y - ty - ay)
        for x, y in reference
        if tx <= x < tx + w and ty <= y < ty + h
    }
    require(
        points and all(0 < x + ax < w - 1 and 0 < y + ay < h - 1 for x, y in points),
        "empty or clipped spacing probe",
    )
    return points


def extract(pages):
    result = {}
    for page, reference in pages:
        require(page.get("font") == "0", "spacing requires resident targets")
        references = {}
        for p in page["probes"]:
            require(
                p["orientation"] == "N" and p["width"] == p["height"],
                "spacing extraction requires upright square probes",
            )
            c = p["character"]
            require(p["text"] == "|" + (c or "") + "|", "incorrect sentinel request")
            if c is None:
                size = p["height"]
                require(size not in references, "duplicate spacing reference")
                points = selected(reference, p)
                right = max(x for x, y in points)
                columns = {x for x, y in points}
                left = right
                while left - 1 in columns:
                    left -= 1
                require(min(columns) < left - 1, "reference sentinels have no gap")
                references[size] = points, left, right
        accounted = set()
        for p in page["probes"]:
            tx, ty, w, h = p["tile"]
            part = {
                (x, y) for x, y in reference if tx <= x < tx + w and ty <= y < ty + h
            }
            require(not (accounted & part), "spacing tiles overlap")
            accounted.update(part)
            if p["character"] is None:
                continue
            size, char = p["height"], p["character"]
            require(size in references, "missing spacing reference")
            original, left, right = references[size]
            measured = selected(reference, p)
            advance = max(x for x, y in measured) - right
            require(advance >= 0, "negative measured advance")
            terminal = {(x, y) for x, y in original if left <= x <= right}
            shifted = {
                (x - advance, y)
                for x, y in measured
                if left + advance <= x <= right + advance
            }
            require(terminal == shifted, "terminal sentinel changed")
            first = {(x, y) for x, y in original if x < left}
            require(first <= measured, "initial sentinel changed")
            key = (char, size)
            require(key not in result, "duplicate spacing observation")
            result[key] = advance
        require(accounted == reference, "unaccounted spacing ink outside native tiles")
    return result


def fit(observations, representative="weighted"):
    require(representative in ("weighted", "midpoint"), "unknown interval representative")
    advances, report = {}, {}
    for char in sorted({c for c, s in observations}):
        samples = sorted((s, a) for (c, s), a in observations.items() if c == char)
        require(len(samples) >= 3, "advance fitting requires at least three sizes")
        # Intersect quantization intervals first. If no exact metric exists,
        # minimize maximum pixel displacement and squared residuals explicitly.
        lower = max((a - 0.5) * 2048 / layout_ppem(s) for s, a in samples)
        upper = min((a + 0.5) * 2048 / layout_ppem(s) for s, a in samples)
        target = (
            sum(s * a / layout_ppem(s) for s, a in samples)
            * 2048
            / sum(s for s, a in samples)
        )
        first, last = max(0, math.ceil(lower)), min(4096, math.ceil(upper) - 1)
        if representative == "midpoint" and first <= last:
            # Integer observations constrain an interval, not an exact scaled
            # coordinate. Its midpoint minimizes worst-case design-unit error
            # over the feasible integers, without adding size-specific values.
            target = (first + last) / 2
        lo = max(
            0, math.floor(min((a - 1.5) * 2048 / layout_ppem(s) for s, a in samples))
        )
        hi = min(
            4096, math.ceil(max((a + 1.5) * 2048 / layout_ppem(s) for s, a in samples))
        )
        require(lo <= hi, "advance outside reconstruction budget")

        def objective(value):
            errors = [layout_advance(value, s) - a for s, a in samples]
            return (
                max(map(abs, errors)),
                sum(e * e for e in errors),
                abs(value - target),
                value,
            )

        winner = min(range(lo, hi + 1), key=objective)
        advances[char] = winner
        report[char] = dict(
            units=winner,
            exact_interval=[lower, upper],
            exact=objective(winner)[0] == 0,
            samples=[
                dict(size=s, measured=a, predicted=layout_advance(winner, s))
                for s, a in samples
            ],
        )
    return advances, report
