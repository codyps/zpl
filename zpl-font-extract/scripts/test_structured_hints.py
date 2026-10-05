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
import split_hint_features

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

    def test_rounding_phase_and_fractional_shift_execute_independently(self):
        state = specimen()
        state["programs"]["H"][0][1] = dict(op="anchor", round="grid")
        self.assertEqual(points(state, 11, 11)[2][0], 128)
        state["programs"]["H"][0][1]["phase"] = 24
        self.assertEqual(points(state, 11, 11)[2][0], 192)
        state["programs"]["H"][0][1]["shift"] = -16
        self.assertEqual(points(state, 11, 11)[2][0], 176)

    def test_fractional_interpolation_shift_is_applied_after_ip(self):
        state = specimen()
        state["programs"]["H"][0][1] = dict(op="interpolate", refs=[0, 2], round="none")
        before = points(state, 11, 11)[2][0]
        state["programs"]["H"][0][1]["shift"] = 12
        self.assertEqual(points(state, 11, 11)[2][0], before + 12)

    def test_numeric_hint_adjustments_are_bounded(self):
        state = specimen()
        for field, value in (("phase", 33), ("shift", -65), ("design", 65)):
            nodes = deepcopy(state["programs"]["H"][0])
            nodes[0][field] = value
            with self.assertRaisesRegex(ValueError, "shared-size bound"):
                hints.ordered(nodes, state["graph"]["H"][0])

    def test_optical_correction_fades_and_restores_movement_axis(self):
        baseline = specimen()
        for sign in (-1, 1):
            state = deepcopy(baseline)
            state["programs"]["H"][0][0].update(
                shift=sign * 64, fade=dict(end=16, measure="y")
            )
            for size, correction in ((8, 64), (10, 64), (13, 32), (16, 0), (24, 0)):
                before, after = points(baseline, 30, size), points(state, 30, size)
                self.assertEqual(after[0][0] - before[0][0], sign * correction)
                self.assertEqual([p[1] for p in after], [p[1] for p in before])

    def test_post_interpolation_adjustment_preserves_anchors_and_expires(self):
        baseline = specimen()
        state = deepcopy(baseline)
        state["programs"]["H"][0][1] = dict(
            op="adjust", round="none", shift=64, fade=dict(end=16, measure="y")
        )
        for size, correction in ((10, 64), (13, 32), (16, 0), (24, 0)):
            before, after = points(baseline, 30, size), points(state, 30, size)
            self.assertEqual(after[2][0] - before[2][0], correction)
            for i in (0, 1, 3, 4):
                self.assertEqual(after[i], before[i])
            if not correction:
                self.assertEqual(after, before)
        state["programs"]["H"][0][0] = dict(op="adjust", round="none", shift=16)
        state["programs"]["H"][0][1] = dict(op="interpolate", refs=[0, 2], round="none")
        with self.assertRaisesRegex(ValueError, "later adjustment"):
            hints.build(state)

    def test_disconnected_feature_split_preserves_coordinates_and_references(self):
        state = specimen()
        state["programs"]["H"][0][1] = dict(op="interpolate", refs=[0, 2], round="none")
        candidate = split_hint_features.split(state)
        self.assertEqual(len(candidate["graph"]["H"][0]["groups"]), 4)
        self.assertEqual(candidate["programs"]["H"][0][3]["refs"], [0, 2])
        self.assertEqual(split_hint_features.split(candidate), candidate)
        for size in (10, 13, 16, 24, 33, 90, 91):
            self.assertEqual(points(candidate, size, 24), points(state, size, 24))
        candidate["programs"]["H"][0][3]["shift"] = 16
        before, after = points(state, 24, 24), points(candidate, 24, 24)
        self.assertEqual(after[2], before[2])
        self.assertEqual(after[5][0], before[5][0] + 16)

    def test_regime_uses_projection_axis_and_restores_fallback(self):
        state = specimen()
        baseline = deepcopy(state)
        nodes = deepcopy(state["programs"]["H"][0])
        nodes[0]["round"] = "half"
        state["regimes"] = {"H": [dict(limit=16, nodes=nodes), None]}
        self.assertNotEqual(points(state, 16, 30), points(baseline, 16, 30))
        self.assertEqual(points(state, 17, 30), points(baseline, 17, 30))
        self.assertEqual(points(state, 16, 100), points(baseline, 16, 100))

    def test_independent_axis_guard_preserves_small_axis_of_stretched_glyph(self):
        state = specimen()
        baseline = deepcopy(state)
        state["programs"]["H"][0][0]["round"] = "half"
        state["independent_axes"] = ["H"]
        self.assertEqual(points(state, 16, 100)[0][0], 32)
        self.assertEqual(points(baseline, 16, 100)[0][0], 0)
        self.assertEqual(points(state, 100, 100), points(baseline, 100, 100))

    def test_second_coarse_regime_uses_both_boundaries(self):
        state = specimen()
        normal = deepcopy(state["programs"]["H"][0])
        small = deepcopy(normal)
        small[0]["shift"] = 16
        tiny = deepcopy(normal)
        tiny[0]["shift"] = 32
        state["regimes"] = {
            "H": [dict(limit=24, nodes=small, smaller=dict(limit=12, nodes=tiny)), None]
        }
        for size, expected in ((11, 32), (12, 32), (13, 16), (24, 16), (25, 0)):
            self.assertEqual(points(state, size, 30)[0][0], expected)
        state["regimes"]["H"][0]["smaller"]["limit"] = 24
        with self.assertRaisesRegex(ValueError, "below its parent"):
            hints.build(state)
        # Equal numeric cutoffs still refine a region when max(X,Y) is used
        # inside an X-only condition, preserving the stretched-size fallback.
        state["regimes"]["H"][0]["smaller"]["measure"] = "max"
        self.assertEqual(points(state, 24, 24)[0][0], 32)
        self.assertEqual(points(state, 24, 30)[0][0], 16)

    def test_hint_range_can_follow_height_or_both_dimensions(self):
        state = specimen()
        small = deepcopy(state["programs"]["H"][0])
        small[0]["shift"] = 16
        state["regimes"] = {"H": [dict(limit=16, nodes=small, measure="y"), None]}
        self.assertEqual(points(state, 30, 16)[0][0], 16)
        self.assertEqual(points(state, 16, 30)[0][0], 0)
        state["regimes"]["H"][0]["measure"] = "max"
        self.assertEqual(points(state, 16, 16)[0][0], 16)
        self.assertEqual(points(state, 16, 30)[0][0], 0)
        state["regimes"]["H"][0]["measure"] = "min"
        self.assertEqual(points(state, 16, 30)[0][0], 16)

    def test_optical_program_preserves_nested_fallback_for_stretched_sizes(self):
        state = specimen()
        small = deepcopy(state["programs"]["H"][0])
        small[0]["shift"] = -16
        tiny = deepcopy(small)
        tiny[0]["shift"] = -32
        state["regimes"] = {
            "H": [dict(limit=24, nodes=small, smaller=dict(limit=16, nodes=tiny)), None]
        }
        baseline = deepcopy(state)
        optical = deepcopy(small)
        optical[0]["shift"] = 64
        state["optical_programs"] = {"H": dict(limit=16, nodes=[optical, None])}
        self.assertEqual(points(state, 16, 16)[0][0], 64)
        for x, y in ((16, 17), (17, 16), (16, 40), (24, 40), (40, 16), (100, 100)):
            self.assertEqual(points(state, x, y), points(baseline, x, y))
        state["optical_programs"]["H"]["limit"] = 10
        self.assertEqual(points(state, 10, 10)[0][0], 64)
        self.assertEqual(points(state, 10, 11), points(baseline, 10, 11))
        self.assertEqual(points(state, 11, 10), points(baseline, 11, 10))
        state["optical_programs"]["H"]["measures"] = ["axis", "max"]
        self.assertEqual(points(state, 10, 30)[0][0], 64)
        self.assertEqual(points(state, 30, 10), points(baseline, 30, 10))

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

    def test_curved_stems_recover_asymmetric_g_bowl_relationships(self):
        root = ROOT / "structured-hints-20261004"
        state = json.loads((root / "candidates.json").read_text())["structured_robust"]
        candidate = structure.curved_stems(state)
        self.assertEqual(hints.build(candidate), hints.build(state))
        self.assertEqual(candidate["programs"], state["programs"])
        pairs = {
            (p["low"], p["high"])
            for p in candidate["graph"]["g"][1]["stems"]
            if p.get("curved")
        }
        self.assertEqual(pairs, {(2, 3), (4, 6)})

    def test_curve_shoulders_preserve_all_existing_programs_and_font_bytes(self):
        root = ROOT / "structured-hints-20261004"
        state = json.loads((root / "candidates.json").read_text())["structured_robust"]
        candidate = structure.curve_shoulders(state)
        self.assertEqual(hints.build(candidate), hints.build(state))
        self.assertEqual(candidate["shapes"], state["shapes"])
        self.assertEqual(candidate["graph"]["H"], state["graph"]["H"])
        self.assertGreater(
            len(candidate["graph"]["g"][0]["groups"]),
            len(state["graph"]["g"][0]["groups"]),
        )
        for char, axes in state["programs"].items():
            for axis, nodes in enumerate(axes):
                self.assertEqual(candidate["programs"][char][axis][: len(nodes)], nodes)
                self.assertTrue(
                    all(
                        n is None
                        for n in candidate["programs"][char][axis][len(nodes) :]
                    )
                )

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
