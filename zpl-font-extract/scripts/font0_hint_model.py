"""Original outline-to-hint research model for resident Font 0.

All geometry comes from the declared 512-dot silhouettes. Hints use TrueType
CVT access, SCFS and IUP to constrain extrema and interpolate outline points:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
No per-size glyph bitmaps or exception tables are generated.
"""

import json
from pathlib import Path
import struct
import subprocess
import tempfile

from analyze_font0_outline import contours
from analyze_font_probe import ink
from capture_font_probe import sha
from compare_swiss import counts, pixels
import font_probe


def load_models(root):
    manifest = json.loads((root / "manifest.json").read_text())
    capture = json.loads((root / "zd621/capture.json").read_text())
    assert (
        capture["status"] == "complete"
        and capture["outline_repeat_exact"]
        and capture["resident_repeat_exact"]
    )
    assert sha((root / "manifest.json").read_bytes()) == capture["manifest_sha256"]
    models = {}
    for page in manifest["pages"]:
        if page["probes"][0]["height"] != 512:
            continue
        path = root / "zd621" / (page["name"] + ".png")
        record = next(p for p in capture["pages"] if p["name"] == page["name"])
        assert sha(path.read_bytes()) == record["png_sha256"]
        points = ink(path, page["canvas"])
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            shape = contours(
                {
                    (x - ax, y - ay)
                    for x, y in points
                    if tx <= x < tx + tw and ty <= y < ty + th
                }
            )
            models[p["text"]] = [[(x * 4, -y * 4, True) for x, y in c] for c in shape]
    return models


def build_font(models, programs=None):
    programs = programs or {}
    items = [
        dict(
            name=f"outline-{code}",
            codepoint=code,
            contours=models.get(chr(code), []),
            instructions=programs.get(chr(code), b"").hex(),
            advance=2048,
            family="pilot",
        )
        for code in range(32, max(map(ord, models)) + 1)
    ]
    # IUP programs may address many points, but never have more than 16 stack operands.
    return font_probe.font(items)


def engine_rows(engine, data, queries, mode="zd621", hinting="native"):
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "candidate.ttf"
        path.write_bytes(data)
        output = subprocess.run(
            [str(engine.resolve()), "glyphs", str(path), mode, hinting],
            input="".join(" ".join(map(str, q)) + "\n" for q in queries),
            text=True,
            capture_output=True,
            check=True,
            timeout=120,
        )
    rows = [json.loads(line) for line in output.stdout.splitlines()]
    for q, row in zip(queries, rows, strict=True):
        if "error" in row:
            raise ValueError((q, row))
    return dict(zip(queries, rows, strict=True))


def load_pages(root, font, group):
    manifest = json.loads((root / "manifest.json").read_text())
    capture = json.loads((root / "zd621/capture.json").read_text())
    assert (
        capture["status"] == "complete"
        and capture["outline_repeat_exact"]
        and capture["resident_repeat_exact"]
    )
    assert sha((root / "manifest.json").read_bytes()) == capture["manifest_sha256"]
    result = []
    for page in manifest["pages"]:
        if page["font"] != font or page["group"] != group:
            continue
        path = root / "zd621" / (page["name"] + ".png")
        record = next(p for p in capture["pages"] if p["name"] == page["name"])
        assert sha(path.read_bytes()) == record["png_sha256"]
        assert (
            sha((root / "zd621" / (page["name"] + ".zpl")).read_bytes())
            == record["zpl_sha256"]
            == page["zpl_sha256"]
        )
        result.append((page, ink(path, page["canvas"])))
    return result


def queries_for(pages):
    return sorted(
        {
            (
                p["width"] or p["height"],
                p["height"],
                ord(p["text"]),
                "NRIB".index(p["orientation"]),
            )
            for page, _ in pages
            for p in page["probes"]
        }
    )


def score(pages, rows):
    report = []
    for page, reference in pages:
        candidate = set()
        cases = []
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            key = (
                p["width"] or p["height"],
                p["height"],
                ord(p["text"]),
                "NRIB".index(p["orientation"]),
            )
            points = {(tx + ax + x, ty + ay + y) for x, y in pixels(rows[key])}
            assert all(
                tx <= x < tx + tw and ty <= y < ty + th for x, y in points
            ), "candidate leaves tile"
            expected = {
                (x, y) for x, y in reference if tx <= x < tx + tw and ty <= y < ty + th
            }
            stats = counts(expected, points)
            cases.append(
                dict(
                    text=p["text"],
                    width=p["width"],
                    height=p["height"],
                    orientation=p["orientation"],
                    **stats,
                )
            )
            candidate.update(points)
        stats = counts(reference, candidate)
        report.append(
            dict(
                name=page["name"],
                **stats,
                iou=1 - stats["xor"] / stats["union"],
                cases=cases,
            )
        )
    return report


def axis_groups(shape, axis, edge_min=80, merge=12):
    """Collect contour extrema and long axis-parallel edges, merge trace jitter."""
    points = [p for c in shape for p in c]
    candidates = set()
    for contour in shape:
        lo = min(p[axis] for p in contour)
        hi = max(p[axis] for p in contour)
        for i, p in enumerate(contour):
            if p[axis] in (lo, hi):
                candidates.add(p[axis])
            q = contour[(i + 1) % len(contour)]
            if p[axis] == q[axis] and abs(p[1 - axis] - q[1 - axis]) >= edge_min:
                candidates.add(p[axis])
    clusters = []
    for value in sorted(candidates):
        if clusters and value - clusters[-1][0] <= merge:
            clusters[-1].append(value)
        else:
            clusters.append([value])
    return [
        dict(
            value=round(sum(c) / len(c)),
            points=[i for i, p in enumerate(points) if p[axis] in c],
        )
        for c in clusters
    ]


def compile_hints(models, hints, parameters, witnesses=False):
    """Compile one coordinate-constraint graph per glyph into original bytecode."""
    cvt = []

    def scaled(value):
        value = round(value)
        if value not in cvt:
            cvt.append(value)
        return font_probe.push(cvt.index(value)) + bytes([0x45])

    def raw_round(value):
        expr = scaled(abs(value)) + bytes([0x68])
        return expr + bytes([0x65]) if value < 0 else expr

    def distance(value, axis):
        stem = 160 <= abs(value) <= 360
        expr = scaled(abs(value))
        shared = parameters["stem_x" if axis == 0 else "stem_y"]
        if stem and shared:
            # MIRP-like cut-in: retain the actual design distance when a shared
            # stem differs by more than the fixed device-space cut-in.
            expr += (
                bytes([0x20])
                + scaled(shared)
                + bytes([0x61, 0x64])
                + font_probe.push(parameters["cutin"])
                + bytes([0x50, 0x58, 0x21])
                + scaled(shared)
                + bytes([0x59])
            )
        if stem and parameters["bias_x" if axis == 0 else "bias_y"]:
            expr += font_probe.push(
                parameters["bias_x" if axis == 0 else "bias_y"]
            ) + bytes([0x60])
        expr += bytes([0x68])
        if stem:
            expr += font_probe.push(64) + bytes([0x8B])
        return expr

    programs = {}
    for char, shape in models.items():
        program = b""
        for axis in (0, 1):
            groups = axis_groups(shape, axis, parameters["edge_min"])
            modes = hints[char][axis]
            if len(modes) != len(groups):
                raise ValueError("hint group count differs from source geometry")
            expressions = {}
            for i, (g, mode) in enumerate(zip(groups, modes)):
                value = g["value"]
                parent = None
                if mode == "none":
                    continue
                if mode == "grid":
                    expr = raw_round(value)
                elif mode == "zone":
                    zone = min([0, 1144, 1532], key=lambda z: abs(value - z))
                    delta = value - zone
                    if abs(delta) == 32:
                        delta = parameters["overshoot"] * (1 if delta > 0 else -1)
                    expr = raw_round(zone) + raw_round(delta) + bytes([0x60])
                elif mode in ("prev", "next") or mode.startswith("link:"):
                    parent = (
                        int(mode.split(":")[1])
                        if mode.startswith("link:")
                        else i + (-1 if mode == "prev" else 1)
                    )
                    if not 0 <= parent < len(groups):
                        raise ValueError("no adjacent group")
                    delta = value - groups[parent]["value"]
                    expr = (
                        font_probe.push(groups[parent]["points"][0])
                        + bytes([0x46])
                        + distance(delta, axis)
                    )
                    if delta < 0:
                        expr += bytes([0x65])
                    expr += bytes([0x60])
                elif mode in ("center_prev", "center_next"):
                    other = i + (-1 if mode == "center_prev" else 1)
                    if not 0 <= other < len(groups):
                        raise ValueError("no centered pair")
                    delta = value - groups[other]["value"]
                    expr = (
                        scaled((value + groups[other]["value"]) / 2)
                        + distance(delta, axis)
                        + font_probe.push(128)
                        + bytes([0x62])
                    )
                    if delta < 0:
                        expr += bytes([0x65])
                    expr += bytes([0x60, 0x68])
                else:
                    raise ValueError(mode)
                expressions[i] = (expr, parent)
            pending = set(expressions)
            while pending:
                ready = [i for i in sorted(pending) if expressions[i][1] not in pending]
                if not ready:
                    raise ValueError("cyclic hint graph")
                for i in ready:
                    expr, _ = expressions[i]
                    for point in groups[i]["points"]:
                        program += (
                            bytes([0x01 if axis == 0 else 0x00])
                            + font_probe.push(point)
                            + expr
                            + bytes([0x48])
                        )
                    pending.remove(i)
            program += bytes([0x31 if axis == 0 else 0x30])
        programs[char] = program
    items = [
        dict(
            name=f"outline-{code}",
            codepoint=code,
            contours=models.get(chr(code), []),
            instructions=programs.get(chr(code), b"").hex(),
            advance=2048,
            family="pilot",
        )
        for code in range(32, max(map(ord, models)) + 1)
    ]
    if witnesses:
        items[1] = font_probe.glyphs()[1]
        items[2] = dict(
            name="instruction-witness",
            codepoint=34,
            contours=[font_probe.rect(0, 0, 1024, 256)],
            instructions=font_probe.set_right(font_probe.push(13 * 64)).hex(),
            advance=1024,
            family="state",
        )
    # These hints need three temporary stack values above SCFS's point argument.
    return font_probe.font(
        items, {b"cvt ": bytearray(struct.pack(">" + str(len(cvt)) + "h", *cvt))}
    )
