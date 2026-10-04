"""Relational hint execution and regression-aware fitting contracts."""

from copy import deepcopy
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

from fit_structured_hints import Objective
import font_experiment_replay as replay
import joint_hint_program as hints
import reconstruct_font as pipeline
import structured_font_hints as structure

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


def specimen():
    shape = [
        [
            (0, 0, True),
            (0, 1000, True),
            (400, 1000, True),
            (1000, 1000, True),
            (1000, 0, True),
            (400, 0, True),
        ]
    ]
    feature = dict(
        groups=[
            dict(value=v, points=p, span=[0, 1000])
            for v, p in [(0, [0, 1]), (400, [2, 5]), (1000, [3, 4])]
        ],
        stems=[],
        counters=[dict(low=0, high=2, width=1000)],
    )
    return dict(
        shapes={"H": shape},
        graph={"H": [feature, dict(groups=[], stems=[])]},
        programs={
            "H": [
                [
                    dict(op="anchor", round="grid"),
                    None,
                    dict(op="anchor", round="ceil"),
                ],
                [],
            ]
        },
        parameters=dict(widths=[[], []], zones=[], cutin=16, limit=90),
        zone_origins=[],
    )


def points(state, x, y):
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "font.ttf"
        path.write_bytes(hints.build(state))
        face = freetype.Face(str(path))
        face.set_pixel_sizes(x, y)
        face.load_char(
            "H",
            freetype.FT_LOAD_TARGET_MONO
            | freetype.FT_LOAD_NO_AUTOHINT
            | freetype.FT_LOAD_PEDANTIC,
        )
        return face.glyph.outline.points


def stats(xor, union):
    return dict(under=xor, over=0, xor=xor, union=union)


class StructuredTests(unittest.TestCase):
    def test_interpolation_preserves_relative_position_between_fitted_anchors(self):
        state = specimen()
        state["programs"]["H"][0][1] = dict(op="interpolate", refs=[0, 2], round="none")
        p = points(state, 11, 11)
        self.assertEqual(p[0][0], 0)
        self.assertEqual(p[3][0], 384)
        self.assertLess(abs(p[2][0] - 0.4 * p[3][0]), 2)
        state["programs"]["H"][0][1]["round"] = "grid"
        self.assertEqual(points(state, 11, 11)[2][0] % 64, 0)

    def test_relative_counter_keeps_unrounded_distance(self):
        state = specimen()
        state["programs"]["H"][0][2] = dict(
            op="relative", ref=0, round="none", minimum=False
        )
        self.assertEqual(points(state, 11, 11)[3][0], 344)

    def test_regime_uses_projection_axis_and_restores_fallback(self):
        state = specimen()
        baseline = deepcopy(state)
        nodes = deepcopy(state["programs"]["H"][0])
        nodes[0]["round"] = "half"
        state["regimes"] = {"H": [dict(limit=16, nodes=nodes), None]}
        self.assertNotEqual(points(state, 16, 30), points(baseline, 16, 30))
        self.assertEqual(points(state, 17, 30), points(baseline, 17, 30))
        self.assertEqual(points(state, 16, 100), points(baseline, 16, 100))

    def test_relations_reject_cycles_and_unbracketed_interpolation(self):
        state = specimen()
        nodes = state["programs"]["H"][0]
        feature = state["graph"]["H"][0]
        nodes[0] = dict(op="relative", ref=2, round="none")
        nodes[2] = dict(op="relative", ref=0, round="none")
        with self.assertRaisesRegex(ValueError, "cyclic"):
            hints.ordered(nodes, feature)
        nodes[0] = dict(op="interpolate", refs=[1, 2], round="none")
        with self.assertRaisesRegex(ValueError, "bracketing"):
            hints.ordered(nodes, feature)

    def test_objective_rejects_better_mean_with_worse_pooled_pixels(self):
        a, b = (17, 17, 72, 0), (18, 18, 72, 0)
        baseline = {a: stats(2, 10), b: stats(20, 100)}
        candidate = {a: stats(0, 10), b: stats(25, 100)}
        _, violations = Objective(baseline, "robust").measure(candidate)
        self.assertIn("glyph-size", violations)
        _, violations = Objective(baseline, "legacy").measure(candidate)
        self.assertEqual(violations, [])

    def test_objective_protects_exact_cases_and_transformation_groups(self):
        a, b = (17, 17, 72, 0), (18, 18, 72, 1)
        baseline = {a: stats(20, 100), b: stats(20, 100)}
        candidate = {a: stats(0, 100), b: stats(25, 100)}
        _, violations = Objective(baseline, "robust").measure(candidate)
        self.assertIn("glyph-transform", violations)
        baseline[b] = stats(0, 100)
        candidate[b] = stats(1, 100)
        _, violations = Objective(baseline, "robust").measure(candidate)
        self.assertIn("exact", violations)

    def test_expansion_preserves_frozen_font_and_detects_o_counter(self):
        model, _ = pipeline.frozen(ROOT / "expanded-optimization-20261004")
        state = structure.initialize(model["state"])
        self.assertEqual(hints.build(state), hints.build(model["state"]))
        self.assertTrue(state["graph"]["O"][0]["counters"])
        self.assertEqual(structure.complexity(state, model["state"]), 0)

    def test_frozen_fonts_recompile_and_execute_across_branch_boundaries(self):
        root = ROOT / "structured-hints-20261004"
        candidates = json.loads((root / "candidates.json").read_text())
        report = json.loads((root / "report.json").read_text())
        plan = json.loads((root / "audit-plan.json").read_text())
        for artifact in plan["frozen_artifacts"]:
            self.assertEqual(
                pipeline.sha((ROOT / artifact["path"]).read_bytes()), artifact["sha256"]
            )
        for label, state in candidates.items():
            path = root / f"{label}.ttf"
            self.assertEqual(hints.build(state), path.read_bytes())
            self.assertEqual(
                pipeline.sha(path.read_bytes()),
                report["variants"][label]["font_sha256"],
            )
            self.assertTrue(report["variants"][label]["cache_audit_exact"])
            self.assertEqual(state["shapes"], candidates["baseline"]["shapes"])
            self.assertEqual(state["parameters"], candidates["baseline"]["parameters"])
            face = freetype.Face(str(path))
            for x, y in [
                (size, size)
                for size in (15, 16, 17, 23, 24, 25, 31, 32, 33, 90, 91, 432)
            ] + [(16, 30), (30, 16), (24, 40), (40, 24), (32, 60), (60, 32)]:
                face.set_pixel_sizes(x, y)
                for char in state["shapes"]:
                    face.load_char(
                        char,
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC,
                    )
                    self.assertGreater(len(face.glyph.outline.points), 0)

    def test_replay_preparation_reproduces_frozen_requests_without_reading_pixels(self):
        fit = ROOT / "structured-hints-20261004"
        for label in ("baseline", "candidate"):
            root = ROOT / f"structured-{label}-replay-20261004"
            manifest = json.loads((root / "manifest.json").read_text())
            with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
                pipeline, "load_pages", side_effect=AssertionError("read pixels")
            ), mock.patch.object(
                replay, "ink", side_effect=AssertionError("read pixels")
            ):
                output = Path(temporary) / "replay"
                replay.prepare(
                    fit,
                    manifest["variant"],
                    ROOT,
                    [ROOT / source["path"] for source in manifest["sources"]],
                    output,
                    manifest["object"],
                )
                self.assertEqual(
                    (output / "probe.ttf").read_bytes(),
                    (root / "probe.ttf").read_bytes(),
                )
                self.assertEqual(
                    json.loads((output / "manifest.json").read_text()), manifest
                )
                for page in manifest["pages"]:
                    name = page["name"] + ".zpl"
                    self.assertEqual(
                        (output / name).read_bytes(), (root / name).read_bytes()
                    )

    def test_paired_printer_comparison_recomputes_native_counts_and_rejects_mismatch(
        self,
    ):
        with tempfile.TemporaryDirectory() as temporary, redirect_stdout(io.StringIO()):
            directory = Path(temporary)
            reports = []
            for label in ("baseline", "candidate"):
                root = ROOT / f"structured-{label}-replay-20261004"
                output = directory / f"{label}.json"
                replay.compare(root, ROOT, output)
                self.assertEqual(
                    json.loads(output.read_text()),
                    json.loads((root / "printer-versus-resident.json").read_text()),
                )
                reports.append(output)
            result = replay.paired(*reports, directory / "paired.json")
            self.assertEqual(
                result,
                json.loads(
                    (
                        ROOT
                        / "structured-hints-20261004/paired-printer-evaluation.json"
                    ).read_text()
                ),
            )
            self.assertTrue(result["passes_baseline_page_gate"])
            self.assertTrue(all(not page["lost_exact"] for page in result["pages"]))
            changed = json.loads(reports[1].read_text())
            changed["pages"][0]["reference_png_sha256"] = "different reference"
            pipeline.save(reports[1], changed)
            with self.assertRaisesRegex(ValueError, "paired resident targets differ"):
                replay.paired(*reports, directory / "invalid.json")


if __name__ == "__main__":
    unittest.main()
