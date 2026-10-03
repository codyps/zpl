#!/usr/bin/env python3
"""Analyze the constructed-font campaign at native origins; no printer access.

State/geometry comparisons use full canvases. Glyph-local measurements and paired
curve/rotation diagnostics use only declared origins and exact integer transforms;
they do not search for alignment or alter the FreeType comparison data.
TrueType state and phantom points:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs
"""
import argparse
from collections import defaultdict
import hashlib
import importlib.util
from importlib.metadata import version
import json
from pathlib import Path
import re

import freetype
from PIL import Image

spec = importlib.util.spec_from_file_location(
    "calibration", Path(__file__).with_name("compare-calibration-freetype.py")
)
calibration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(calibration)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def ink(path, dimensions):
    with Image.open(path) as source:
        if list(source.size) != dimensions:
            raise ValueError("Native canvas mismatch")
        im = source.convert("L")
    pixels = list(im.get_flattened_data())
    if not set(pixels) <= {0, 255}:
        raise ValueError("Non-binary input")
    return {(i % im.width, i // im.width) for i, p in enumerate(pixels) if p == 0}


def within(points, probe):
    x, y, w, h = probe["tile"]
    return {(px, py) for px, py in points if x <= px < x + w and y <= py < y + h}


def glyph_coordinates(points, probe):
    # Exact inverse of the planned transform, only for paired diagnostics.
    # No translation is inferred from observed bounds or optimized for agreement.
    tx, ty, _, _ = probe["tile"]
    ax, ay = probe["anchor"]
    normalized = set()
    for x, y in points:
        dx, dy = x - tx - ax, y - ty - ay
        u, v = {
            "N": (dx, dy),
            "R": (dy, -dx - 1),
            "I": (-dx - 1, -dy - 1),
            "B": (-dy - 1, dx),
        }[probe["orientation"]]
        normalized.add((u, v))
    return normalized


def measure(points, probe):
    normalized = glyph_coordinates(points, probe)
    if not normalized:
        return dict(ink=0, bounds=None, column_runs=[])
    bounds = [
        min(x for x, y in normalized),
        min(y for x, y in normalized),
        max(x for x, y in normalized) + 1,
        max(y for x, y in normalized) + 1,
    ]
    columns = sorted({x for x, y in normalized})
    runs = []
    for x in columns:
        if runs and runs[-1][1] == x:
            runs[-1][1] = x + 1
        else:
            runs.append([x, x + 1])
    return dict(ink=len(normalized), bounds=bounds, column_runs=runs)


def analyze(root, devices):
    manifest = json.loads((root / "manifest.json").read_text())
    if sha(root / "probe.ttf") != manifest["font_sha256"]:
        raise ValueError("Font changed")
    report = dict(
        schema="constructed-font-analysis-v1",
        freetype_version=freetype.version(),
        python_packages={n: version(n) for n in ("freetype-py", "Pillow")},
        manifest_sha256=sha(root / "manifest.json"),
        font_sha256=manifest["font_sha256"],
        load_flags=calibration.MODES["native"],
        coordinate_policy="original full canvases, without alignment, clipping, or resampling",
        paired_diagnostics="declared glyph origins and exact integer rotations; no fitted offsets",
        face_lifecycle="new FreeType face for each glyph",
        sealed_pixels_read=False,
        devices={},
        cross_device=[],
    )
    prepared = {
        p["name"]: (root / (p["name"] + ".zpl")).read_bytes() for p in manifest["pages"]
    }
    for page in manifest["pages"]:
        if hashlib.sha256(prepared[page["name"]]).hexdigest() != page["zpl_sha256"]:
            raise ValueError("Prepared request changed")
    points_by_device = {}
    for name in devices:
        directory = root / name
        capture = json.loads((directory / "capture.json").read_text())
        if capture["manifest_sha256"] != sha(root / "manifest.json"):
            raise ValueError("Capture manifest changed")
        cleanup = capture.get("cleanup", {})
        recovery_path = directory / "cleanup-recovery.json"
        recovered = False
        if capture["status"] == "cleanup-failed" and recovery_path.exists():
            recovery = json.loads(recovery_path.read_text())
            recovered = (
                recovery["capture_sha256"] == sha(directory / "capture.json")
                and recovery["font_sha256"] == manifest["font_sha256"]
                and recovery["printer"]
                == {k: v for k, v in capture["printer"].items() if k != "dpi"}
                and recovery["object"] == manifest["object"]
                and recovery["confirmed_absent"]
            )
        if not (
            (capture["status"] == "complete" and cleanup.get("confirmed_absent"))
            or recovered
        ):
            raise ValueError("Incomplete capture or cleanup")
        if not capture.get("resident_repeat_exact") or not all(
            p["status"] == "captured" for p in capture["pages"]
        ):
            raise ValueError("Capture controls or page completion failed")
        by_page = {p["name"]: p for p in capture["pages"]}
        if set(by_page) != set(prepared) | {"resident-start", "resident-end"} or len(
            by_page
        ) != len(capture["pages"]):
            raise ValueError("Unexpected capture pages")
        if not capture.get("font_selection_and_execution_verified"):
            raise ValueError("Font execution control failed")
        before = capture.get("label_length_settings_before")
        if before is not None and capture.get("label_length_settings_after") != before:
            raise ValueError("Printer settings were not restored")
        for p in capture["pages"]:
            for extension, key in [("zpl", "zpl_sha256"), ("png", "png_sha256")]:
                if sha(directory / (p["name"] + "." + extension)) != p[key]:
                    raise ValueError("Capture source hash differs")
            if p["name"] in prepared:
                expected = prepared[p["name"]]
                if before is not None:
                    expected, n = re.subn(
                        rb"\^LL([0-9]+)(?=\^)", rb"^LL\1,Y", expected, count=1
                    )
                    if n != 1:
                        raise ValueError("Missing label length")
                if (directory / (p["name"] + ".zpl")).read_bytes() != expected:
                    raise ValueError("Submitted request differs from prepared geometry")
        if ink(directory / "resident-start.png", [384, 192]) != ink(
            directory / "resident-end.png", [384, 192]
        ):
            raise ValueError("Resident repeat pixels differ")
        output = dict(
            printer=capture["printer"],
            resident_repeat_exact=capture["resident_repeat_exact"],
            capture_sha256=sha(directory / "capture.json"),
            cleanup_recovery_sha256=sha(recovery_path) if recovered else None,
            states=[],
            metrics=[],
            pages=[],
            equivalent_curves=[],
            bitmap_rotation=[],
            totals={},
        )
        points_by_device[name] = {}
        for page in manifest["pages"]:
            original = ink(directory / (page["name"] + ".png"), page["canvas"])
            points_by_device[name][page["name"]] = original
            candidate = set()
            covered = set()
            glyphs = []
            measured = []
            for p in page["probes"]:
                reference = within(original, p)
                covered |= reference
                measurement = measure(reference, p)
                measured.append(
                    dict(name=p["name"], origin=p["origin"], measurement=measurement)
                )
                if page["group"] == "metrics":
                    continue
                actual, advance = calibration.render_glyph(
                    root / "probe.ttf", p, calibration.MODES["native"]
                )
                candidate |= actual
                glyphs.append(
                    dict(
                        name=p["name"],
                        **calibration.differences(reference, actual),
                        reference=measurement,
                        local=measure(actual, p),
                        advance_26_6=advance,
                    )
                )
            if covered != original:
                raise ValueError("Ink outside declared probe tiles")
            first = page["probes"][0]
            common = dict(
                page=page["name"],
                height=first["height"],
                width=first["width"],
                orientation=first["orientation"],
            )
            if page["group"] == "state":
                output["states"].append(dict(**common, glyphs=glyphs))
            if page["group"] == "metrics":
                for origin in ("FT", "FO"):
                    records = [r for r in measured if r["origin"] == origin]

                    def distance(row):
                        runs = row["measurement"]["column_runs"]
                        if len(runs) < 2:
                            raise ValueError("Missing metric sentinel")
                        return runs[-1][0] - runs[0][0]

                    baseline = distance(
                        next(r for r in records if r["name"] == "sentinel-pair")
                    )
                    for r in records:
                        if r["name"] == "sentinel-pair":
                            continue
                        glyph, n = r["name"].rsplit("-n", 1)
                        n = int(n)
                        g = next(g for g in manifest["glyphs"] if g["name"] == glyph)
                        face = freetype.Face(str(root / "probe.ttf"))
                        face.set_pixel_sizes(first["width"], first["height"])
                        face.load_char(chr(g["codepoint"]), calibration.MODES["native"])
                        output["metrics"].append(
                            dict(
                                **common,
                                origin=origin,
                                glyph=glyph,
                                repetitions=n,
                                sentinel_base=baseline,
                                sentinel_distance=distance(r),
                                measured_advance=(distance(r) - baseline) / n,
                                local_advance=face.glyph.advance.x / 64,
                            )
                        )
            else:
                output["pages"].append(
                    dict(
                        **common,
                        group=page["group"],
                        **calibration.differences(original, candidate),
                        glyphs=glyphs,
                        exact_glyphs=sum(g["xor"] == 0 for g in glyphs),
                    )
                )
            if page["group"] == "geometry":
                by_name = {p["name"]: p for p in page["probes"]}
                for phase in (0, 32, 33):
                    if f"curve-whole-{phase}" not in by_name:
                        continue
                    a, b = (
                        by_name[f"curve-{kind}-{phase}"] for kind in ("whole", "split")
                    )
                    ap, bp = (glyph_coordinates(within(original, p), p) for p in (a, b))
                    output["equivalent_curves"].append(
                        dict(**common, phase=phase, **calibration.differences(ap, bp))
                    )
                if first["orientation"] != "N":
                    normal = next(
                        q
                        for q in manifest["pages"]
                        if q["group"] == "geometry"
                        and q["probes"][0]["orientation"] == "N"
                        and all(
                            q["probes"][0][k] == first[k] for k in ("height", "width")
                        )
                    )
                    source = points_by_device[name][normal["name"]]
                    rotated = set()
                    for p, q in zip(normal["probes"], page["probes"], strict=True):
                        if p["name"] != q["name"]:
                            raise ValueError("Rotation pair differs")
                        tx, ty, _, _ = q["tile"]
                        ax, ay = q["anchor"]
                        for u, v in glyph_coordinates(within(source, p), p):
                            dx, dy = {
                                "R": (-v - 1, u),
                                "I": (-u - 1, -v - 1),
                                "B": (v, -u - 1),
                            }[q["orientation"]]
                            rotated.add((tx + ax + dx, ty + ay + dy))
                    output["bitmap_rotation"].append(
                        dict(
                            **common,
                            source_page=normal["name"],
                            **calibration.differences(original, rotated),
                        )
                    )
        for group in ("state", "geometry"):
            rows = [r for r in output["pages"] if r["group"] == group]
            totals = {
                k: sum(r[k] for r in rows)
                for k in ("underpaint", "overpaint", "xor", "union", "exact_glyphs")
            }
            totals["glyph_count"] = sum(len(r["glyphs"]) for r in rows)
            totals["foreground_iou"] = 1 - totals["xor"] / totals["union"]
            output["totals"][group] = totals
        report["devices"][name] = output
    if len(devices) == 2:
        for page in manifest["pages"]:
            a, b = (points_by_device[d][page["name"]] for d in devices)
            report["cross_device"].append(
                dict(page=page["name"], **calibration.differences(a, b))
            )
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--devices", nargs="+", default=["zd621", "zq610"])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = analyze(args.root, args.devices)
    with args.output.open("x") as f:
        json.dump(report, f, indent=2)
        f.write("\n")
    for name, d in report["devices"].items():
        print(name, d["totals"])
        for row in d["states"]:
            values = {
                g["name"]: g["reference"]["bounds"][2] - g["reference"]["bounds"][0]
                for g in row["glyphs"]
                if g["name"] != "identity"
            }
            print(row["page"], row["height"], row["width"], row["orientation"], values)
        metrics = defaultdict(set)
        for row in d["metrics"]:
            metrics[(row["height"], row["glyph"])].add(row["measured_advance"])
        print("Measured advances", dict(metrics))
    if report["cross_device"]:
        print("Cross-device XOR", sum(p["xor"] for p in report["cross_device"]))
