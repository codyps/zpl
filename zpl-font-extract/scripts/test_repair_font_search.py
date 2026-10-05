"""Cross-axis feature coverage and non-greedy, constraint-preserving repair."""

from copy import deepcopy
from contextlib import redirect_stdout
import io
import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from fit_font_target import metrics
import joint_hint_program as hints
from fit_structured_hints import Oracle, projection_error
import diagonal_font_hints
import font_probe
import line_vector_probe
import font_experiment_replay
from projection_hint_search import repair as projection_repair
import reconstruct_font as pipeline
from repair_font_search import complete_axes, Constraints, identity, search
from test_structured_hints import points, specimen, stats


class RepairTests(unittest.TestCase):
    def test_vector_control_reproduces_requests_and_uses_owned_object(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "probe"
            line_vector_probe.prepare(root)
            for path in root.iterdir():
                self.assertEqual(
                    path.read_bytes(),
                    (fixtures / "line-vector-20261005" / path.name).read_bytes(),
                )
            manifest = json.loads((root / "manifest.json").read_text())
            self.assertEqual(manifest["pages"][0]["name"], "00-state")
            for page in manifest["pages"]:
                data = (root / (page["name"] + ".zpl")).read_bytes()
                self.assertIn(manifest["object"].encode(), data)
                self.assertNotIn(font_probe.OBJECT.encode(), data)

    def test_cached_projection_and_diagonal_branches_keep_pixel_identity(self):
        state = self.diagonal_specimen()
        inactive, active = (10, 10, 72, 0), (24, 24, 72, 1)

        class Engine:
            def __init__(self):
                self.calls = []

            def render(self, font, queries):
                self.calls.append((font, queries))
                return {
                    q: dict(left=len(self.calls), top=0, width=1, rows=["80"])
                    for q in queries
                }

        engine = Engine()
        oracle = Oracle(
            engine, {q: {(0, 0)} for q in (inactive, active)}, state, projections=True
        )
        initial = deepcopy(oracle.last_projections)
        corrected = deepcopy(state)
        corrected["diagonal_programs"] = {"H": dict(start=12, limit=48, shift=16)}
        oracle.stats(corrected)
        self.assertEqual(engine.calls[-1][1], [active])
        self.assertEqual(oracle.last_projections[inactive], initial[inactive])
        self.assertNotEqual(oracle.last_projections[active][1], initial[active][1])
        # Rotation maps the native horizontal displacement to logical Y.
        self.assertEqual(oracle.last_projections[active][0], [0, 1])
        calls = len(engine.calls)
        oracle.stats(corrected)
        self.assertEqual(len(engine.calls), calls)
        corrected["diagonal_programs"]["H"]["shift"] = -16
        oracle.stats(corrected)
        self.assertEqual(len(engine.calls), calls + 1)

    def test_frozen_repair_preserves_geometry_advances_and_replay(self):
        root = (
            Path(__file__).resolve().parents[1] / "tests/fixtures/repair-font-20261005"
        )
        variants = json.loads((root / "candidates.json").read_text())
        for name, state in variants.items():
            self.assertEqual(hints.build(state), (root / (name + ".ttf")).read_bytes())
        for key in ("shapes", "parameters", "advances"):
            self.assertEqual(variants["baseline"][key], variants["targeted"][key])
        self.assertEqual(len(variants["targeted"]["advances"]), 95)

    def test_repair_previews_reproduce_and_printer_evidence_rescores(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        fit = fixtures / "repair-font-20261005"
        captures = [
            fixtures / f"repair-validation-20261005-{name}"
            for name in ("weak", "ascii")
        ]
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            temporary = Path(directory)
            for variant, object_name in (
                ("baseline", "R:ZP26S.TTF"),
                ("targeted", "R:ZP26T.TTF"),
            ):
                root = fixtures / f"repair-{variant}-replay-20261005"
                output = temporary / variant
                font_experiment_replay.prepare(
                    fit, variant, fixtures, captures, output, object_name
                )
                for path in output.iterdir():
                    self.assertEqual(path.read_bytes(), (root / path.name).read_bytes())
                report = temporary / f"{variant}.json"
                font_experiment_replay.compare(root, fixtures, report)
                self.assertEqual(
                    json.loads(report.read_text()),
                    json.loads(
                        (fit / f"{variant}-printer-evaluation.json").read_text()
                    ),
                )
            paired = temporary / "paired.json"
            font_experiment_replay.paired(
                temporary / "baseline.json", temporary / "targeted.json", paired
            )
            measured = json.loads(paired.read_text())
            self.assertEqual(
                measured,
                json.loads((fit / "paired-printer-evaluation.json").read_text()),
            )
            self.assertFalse(measured["passes_baseline_page_gate"])

    def diagonal_specimen(self):
        state = specimen()
        state["shapes"]["H"] = [
            [(0, 0, True), (1000, 1000, True), (1150, 850, True), (150, -150, True)]
        ]
        state["graph"]["H"] = [dict(groups=[], stems=[]), dict(groups=[], stems=[])]
        state["programs"]["H"] = [[], []]
        return state

    def test_diagonal_correction_follows_scaled_normal_in_freetype(self):
        state = self.diagonal_specimen()
        self.assertEqual(len(diagonal_font_hints.strokes(state["shapes"]["H"])), 1)
        fitted = deepcopy(state)
        fitted["diagonal_programs"] = {"H": dict(start=12, limit=48, shift=16)}
        for x, y in ((20, 20), (20, 30), (30, 20)):
            before, after = points(state, x, y), points(fitted, x, y)
            dx, dy = (before[1][a] - before[0][a] for a in (0, 1))
            length = math.hypot(dx, dy)
            # Clockwise contour: positive shift contracts both sides toward ink.
            expected = (dy * 16 / length, -dx * 16 / length)
            for i, sign in ((0, 1), (1, 1), (2, -1), (3, -1)):
                for axis in (0, 1):
                    self.assertAlmostEqual(
                        after[i][axis] - before[i][axis], sign * expected[axis], delta=1
                    )
        for x, y in ((12, 20), (20, 12), (49, 20), (20, 49)):
            self.assertEqual(points(state, x, y), points(fitted, x, y))
        self.assertNotEqual(identity(state, "H"), identity(fitted, "H"))

    def test_diagonal_correction_rejects_unbounded_or_non_stroke_edits(self):
        state = self.diagonal_specimen()
        for config in (
            dict(start=12, limit=48, shift=33),
            dict(start=12, limit=48, shift=1.5),
            dict(start=13, limit=48, shift=16),
            dict(start=24, limit=24, shift=16),
        ):
            state["diagonal_programs"] = {"H": config}
            with self.assertRaises(ValueError):
                hints.build(state)
        self.assertFalse(diagonal_font_hints.strokes(specimen()["shapes"]["H"]))

    def test_axis_completion_preserves_font_and_exposes_diagonal_tip(self):
        root = Path(__file__).resolve().parents[1] / "tests/fixtures"
        original = json.loads(
            (root / "ascii-reconstruction-20261004/candidates.json").read_text()
        )["targeted"]
        completed = complete_axes(original)
        self.assertEqual(hints.build(completed), hints.build(original))
        self.assertEqual(complete_axes(completed), completed)
        self.assertEqual(completed["shapes"], original["shapes"])
        self.assertEqual(completed["parameters"], original["parameters"])
        for c in original["shapes"]:
            for axis in (0, 1):
                old = original["graph"][c][axis]["groups"]
                self.assertEqual(completed["graph"][c][axis]["groups"][: len(old)], old)
        self.assertEqual(len(original["graph"]["<"][1]["groups"]), 2)
        self.assertNotIn(
            4, {p for g in original["graph"]["<"][1]["groups"] for p in g["points"]}
        )
        self.assertIn(
            4, {p for g in completed["graph"]["<"][1]["groups"] for p in g["points"]}
        )

    def test_new_control_moves_the_missing_axis_in_an_independent_interpreter(self):
        state = complete_axes(specimen())
        before = points(state, 17, 17)
        group = state["graph"]["H"][1]["groups"][0]
        state["programs"]["H"][1][0] = dict(op="adjust", round="none", shift=32)
        after = points(state, 17, 17)
        # With one touched Y point, IUP translates its whole contour. X keeps
        # its independent existing anchors.
        for i in range(len(before)):
            self.assertEqual(after[i][0], before[i][0])
            self.assertEqual(after[i][1] - before[i][1], 32)

    def test_search_can_cross_a_regression_without_returning_it(self):
        initial = specimen()
        initial["programs"]["H"][1] = deepcopy(initial["programs"]["H"][0])
        q = (16, 16, 72, 0)

        class Barrier:
            def stats(self, state):
                moves = sum(
                    nodes[0].get("shift", 0) == 64 for nodes in state["programs"]["H"]
                )
                return {q: stats([30, 40, 5][moves], 100)}

        def neighbors(state, char, salt, budget):
            for axis in (0, 1):
                candidate = deepcopy(state)
                candidate["programs"][char][axis][0]["shift"] = 64
                yield candidate

        oracle = Barrier()
        with mock.patch("repair_font_search.candidates", side_effect=neighbors):
            best, measured, history = search(initial, initial, oracle, 3, 3, 0)
        self.assertTrue(metrics(measured)["passes_target"])
        self.assertGreater(
            sum(r["counts"].get("accepted_after_regression", 0) for r in history), 0
        )
        self.assertFalse(
            Constraints(oracle.stats(initial), oracle.stats(initial)).measure(
                oracle.stats(best)
            )[1]
        )
        with mock.patch("repair_font_search.candidates", side_effect=neighbors):
            stopped, measured, _ = search(initial, initial, oracle, 1, 3, 0)
        self.assertEqual(stopped, initial)
        self.assertEqual(measured, oracle.stats(initial))

    def test_equal_error_counts_do_not_collapse_different_programs(self):
        first = specimen()
        second = deepcopy(first)
        first["programs"]["H"][0][0]["shift"] = 8
        second["programs"]["H"][0][0]["shift"] = -8
        self.assertNotEqual(identity(first, "H"), identity(second, "H"))

    def test_projection_guide_separates_displacements_without_changing_native_iou(self):
        reference = {(1, 1), (1, 2)}
        horizontal = {(0, 1), (0, 2)}
        vertical = {(1, 0), (1, 1)}
        self.assertEqual(projection_error(reference, horizontal, 1), 0)
        self.assertEqual(projection_error(reference, vertical, 0), 0)
        self.assertGreater(projection_error(reference, horizontal, 0), 0)
        self.assertGreater(projection_error(reference, vertical, 1), 0)
        self.assertGreater(pipeline.counts(reference, horizontal)["xor"], 0)

    def test_projection_search_combines_repairs_that_have_no_individual_iou_gain(self):
        state = specimen()
        state["programs"]["H"][1] = deepcopy(state["programs"]["H"][0])
        q = (16, 16, 72, 0)
        reference = {(1, 1)}

        class RasterOracle:
            projections = True

            def stats(self, candidate):
                x, y = (
                    nodes[0].get("shift", 0) // 64
                    for nodes in candidate["programs"]["H"]
                )
                pixels = {(x, y)}
                self.last_projections = {
                    q: (
                        [projection_error(reference, pixels, a) for a in (0, 1)],
                        str(pixels),
                    )
                }
                return {q: pipeline.counts(reference, pixels)}

        def options(state, char, axis, branch):
            nodes = deepcopy(state["programs"][char][axis])
            yield deepcopy(nodes)
            nodes[0]["shift"] = 64
            yield nodes

        oracle = RasterOracle()
        baseline = oracle.stats(state)
        constraints = Constraints(baseline, baseline)
        with mock.patch("projection_hint_search.templates", side_effect=options):
            fitted, scores, history = projection_repair(
                state, state, oracle, constraints, beam=2, rounds=1
            )
        self.assertEqual(scores[q]["xor"], 0)
        self.assertTrue(history[0]["passes_target"])
        self.assertFalse(constraints.measure(scores)[1])

    def test_historical_and_improved_exact_cases_both_remain_protected(self):
        a, b = (16, 16, 72, 0), (17, 17, 72, 0)
        original = {a: stats(0, 100), b: stats(20, 100)}
        incumbent = {a: stats(0, 100), b: stats(0, 100)}
        constraints = Constraints(original, incumbent)
        self.assertFalse(constraints.measure(incumbent)[1])
        regressed = {a: stats(0, 100), b: stats(1, 100)}
        self.assertIn("1:exact", constraints.measure(regressed)[1])
        self.assertGreater(constraints.excess(regressed)[0], 0)


if __name__ == "__main__":
    unittest.main()
