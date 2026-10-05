"""Per-case target accounting and metadata-only independent audit planning."""

import json
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from types import SimpleNamespace
from contextlib import redirect_stdout
import io

import freetype

from fit_font_target import (
    copy_hint_state,
    joint_repair,
    metrics,
    proposals,
    screen_reject,
    target_score,
)
from fit_structured_hints import Objective, Oracle
import joint_hint_program as hints
import reconstruct_font as pipeline
from target_font_probe import prepare
from test_structured_hints import specimen, stats


class TargetTests(unittest.TestCase):
    def test_hint_only_copies_preserve_proposals_and_parent_programs(self):
        state = specimen()
        original = deepcopy(state)
        fast = list(proposals(state, "H", 0, 16, True))
        with mock.patch(
            "fit_font_target.copy_hint_state", side_effect=lambda s, c: deepcopy(s)
        ):
            full = list(proposals(state, "H", 0, 16, True))
        self.assertEqual(fast, full)
        self.assertEqual(state, original)
        owned = copy_hint_state(state, "H")
        owned["programs"]["H"][0][0]["shift"] = 16
        self.assertEqual(state, original)
        self.assertIs(owned["shapes"], state["shapes"])

    def test_joint_search_crosses_axis_barrier_without_accepting_regression(self):
        state = specimen()
        state["graph"]["H"][1] = deepcopy(state["graph"]["H"][0])
        state["programs"]["H"][1] = deepcopy(state["programs"]["H"][0])
        query = (10, 10, 72, 0)

        class BarrierOracle:
            keys = {"H": [query]}
            engine = SimpleNamespace(seen=set())

            def stats(self, candidate, queries=None):
                moved = 0
                for axis, nodes in enumerate(candidate["programs"]["H"]):
                    regime = candidate.get("regimes", {}).get("H", [None, None])[axis]
                    if regime:
                        nodes = regime.get("smaller", regime)["nodes"]
                    optical = candidate.get("optical_programs", {}).get("H")
                    if optical and optical["nodes"][axis] is not None:
                        nodes = optical["nodes"][axis]
                    moved += nodes[0].get("shift", 0) == 64
                return {query: stats([30, 40, 5][moved], 100)}

        def templates(candidate, char, axis):
            nodes = deepcopy(candidate["programs"][char][axis])
            yield deepcopy(nodes)
            nodes[0]["shift"] = 64
            yield nodes

        oracle = BarrierOracle()
        objective = Objective(oracle.stats(state), "robust")
        with (
            mock.patch("fit_font_target.axis_templates", side_effect=templates),
            mock.patch("fit_font_target.node_proposals", return_value=[]),
            redirect_stdout(io.StringIO()),
        ):
            candidate, stages = joint_repair(state, oracle, objective, state)
        self.assertTrue(metrics(oracle.stats(candidate))["passes_target"])
        self.assertEqual(objective.measure(oracle.stats(candidate))[1], [])
        self.assertTrue(stages[0]["passes_target"])

    def test_new_range_can_start_with_one_local_edit_and_preserve_fallback(self):
        state = specimen()
        candidates = proposals(state, "H", 0, 16, True)
        found = False
        for candidate in candidates:
            regime = candidate.get("regimes", {}).get("H", [None, None])[0]
            if (
                regime
                and candidate["programs"] == state["programs"]
                and regime["nodes"][0]
                and regime["nodes"][0].get("shift") == 16
                and regime["nodes"][1:] == state["programs"]["H"][0][1:]
            ):
                found = True
                break
        self.assertTrue(found)

    def test_screening_only_rejects_proven_per_case_constraints(self):
        q = (17, 17, 72, 0)
        baseline = {q: stats(20, 100)}
        self.assertFalse(screen_reject({q: stats(26, 100)}, baseline))
        self.assertTrue(screen_reject({q: stats(27, 100)}, baseline))
        baseline[q] = stats(0, 100)
        self.assertTrue(screen_reject({q: stats(1, 100)}, baseline))

    def test_active_program_cache_matches_independent_execution_and_partial_queries(
        self,
    ):
        class Interpreter:
            def __init__(self, path):
                self.path, self.calls = path, 0

            def render(self, data, queries):
                self.calls += 1
                self.path.write_bytes(data)
                face = freetype.Face(str(self.path))
                result = {}
                for q in queries:
                    face.set_char_size(
                        round(pipeline.ppem(q[0]) * 64),
                        round(pipeline.ppem(q[1]) * 64),
                        72,
                        72,
                    )
                    face.load_char(
                        chr(q[2]),
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC
                        | freetype.FT_LOAD_RENDER,
                    )
                    bitmap = face.glyph.bitmap
                    result[q] = dict(
                        left=face.glyph.bitmap_left,
                        top=-face.glyph.bitmap_top,
                        width=bitmap.width,
                        rows=[
                            bytes(
                                bitmap.buffer[y * bitmap.pitch : (y + 1) * bitmap.pitch]
                            ).hex()
                            for y in range(bitmap.rows)
                        ],
                    )
                return result

        with tempfile.TemporaryDirectory() as directory:
            interpreter = Interpreter(Path(directory) / "font.ttf")
            state = specimen()
            queries = [
                (x, y, 72, 0)
                for x, y in (
                    (12, 31),
                    (31, 12),
                    (12, 12),
                    (13, 21),
                    (21, 13),
                    (16, 100),
                    (100, 16),
                    (110, 100),
                )
            ]
            references = {
                q: pipeline.pixels(row)
                for q, row in interpreter.render(hints.build(state), queries).items()
            }
            oracle = Oracle(interpreter, references, state)
            candidate = deepcopy(state)
            candidate["independent_axes"] = ["H"]
            candidate["programs"]["H"][0][0]["shift"] = 64
            small = deepcopy(candidate["programs"]["H"][0])
            small[0]["shift"] = -64
            tiny = deepcopy(small)
            tiny[0]["shift"] = 32
            tiny[0]["fade"] = dict(end=24, measure="x")
            tiny[1] = dict(
                op="adjust", round="none", shift=-32, fade=dict(end=24, measure="y")
            )
            candidate["regimes"] = {
                "H": [
                    dict(
                        limit=24,
                        nodes=small,
                        smaller=dict(limit=12, nodes=tiny, measure="y"),
                    ),
                    None,
                ]
            }
            candidate["optical_programs"] = {
                "H": dict(limit=16, nodes=[deepcopy(tiny), None])
            }
            for measure in ("axis", "x", "y", "min", "max"):
                candidate["regimes"]["H"][0]["measure"] = measure
                candidate["optical_programs"]["H"]["measures"] = [measure, "max"]
                expected = {
                    q: pipeline.counts(references[q], pipeline.pixels(row))
                    for q, row in interpreter.render(
                        hints.build(candidate), queries
                    ).items()
                }
                self.assertEqual(
                    oracle.stats(candidate, queries[:3]),
                    {q: expected[q] for q in queries[:3]},
                )
                self.assertEqual(oracle.stats(candidate), expected)
                calls = interpreter.calls
                self.assertEqual(oracle.stats(candidate), expected)
                self.assertEqual(interpreter.calls, calls)
            # A reused node array with different feature membership/origins is
            # a different program, as happens when a warm start is expanded.
            candidate["graph"]["H"][0]["groups"][0]["hint_origin"] = 128
            expected = {
                q: pipeline.counts(references[q], pipeline.pixels(row))
                for q, row in interpreter.render(
                    hints.build(candidate), queries
                ).items()
            }
            self.assertEqual(oracle.stats(candidate), expected)

    def test_large_exact_glyph_does_not_hide_small_failure(self):
        result = metrics(
            {(10, 10, 72, 0): stats(2, 10), (200, 200, 72, 0): stats(0, 1000)}
        )
        self.assertGreater(result["pooled_iou"], 0.99)
        self.assertEqual(result["at_least_90"], 1)
        self.assertAlmostEqual(result["worst_iou"], 0.8)

    def test_target_penalty_distinguishes_equal_aggregate_errors(self):
        a, b = (17, 17, 72, 0), (18, 18, 72, 0)
        baseline = {a: stats(20, 100), b: stats(20, 100)}
        uneven = {a: stats(16, 100), b: stats(2, 100)}
        both_pass = {a: stats(9, 100), b: stats(9, 100)}
        objective = Objective(baseline, "robust")
        state = specimen()
        self.assertLess(
            target_score(both_pass, objective, state, state),
            target_score(uneven, objective, state, state),
        )
        self.assertEqual(metrics(uneven)["at_least_90"], 1)
        self.assertEqual(metrics(both_pass)["at_least_90"], 2)

    def test_all_cases_target_has_priority_over_better_average(self):
        a, b = (17, 17, 72, 0), (18, 18, 72, 0)
        baseline = {a: stats(20, 100), b: stats(20, 100)}
        excellent_average = {a: stats(11, 100), b: stats(0, 100)}
        both_pass = {a: stats(10, 100), b: stats(10, 100)}
        objective = Objective(baseline, "robust")
        state = specimen()
        self.assertLess(
            target_score(both_pass, objective, state, state),
            target_score(excellent_average, objective, state, state),
        )

    def test_audit_plan_excludes_old_requests_and_requires_no_images(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixtures = root / "fixtures"
            fixtures.mkdir()
            (fixtures / "manifest.json").write_text(
                json.dumps(
                    dict(
                        schema="small-font-probe-v1",
                        pages=[
                            dict(
                                font="0",
                                probes=[dict(width=15, height=15, orientation="N")],
                            )
                        ],
                    )
                )
            )
            output = root / "planned"
            prepare(fixtures, output)
            plan = json.loads((output / "plan.json").read_text())
            seen = set()
            cases = 0
            for campaign in plan["campaigns"]:
                for q in campaign["configurations"]:
                    self.assertNotIn(tuple(q), seen)
                    self.assertNotEqual(tuple(q), (15, 15, "N"))
                    seen.add(tuple(q))
                manifest = json.loads(
                    (output / campaign["path"] / "manifest.json").read_text()
                )
                for page in manifest["pages"]:
                    cases += len(page["probes"])
                    self.assertLessEqual(
                        page["canvas"][0] * page["canvas"][1], 4_000_000
                    )
            self.assertEqual(len(seen), 64)
            self.assertEqual(cases, 384)
            self.assertEqual(plan["cases"], cases)
            prepare(fixtures, root / "minimum", minimum_controls=True)
            controls = json.loads((root / "minimum/plan.json").read_text())
            self.assertEqual(controls["cases"], 72)
            self.assertIn("controls", controls["role"])
            self.assertTrue(
                all(
                    1 <= w <= 10 and 1 <= h <= 10
                    for c in controls["campaigns"]
                    for w, h, _ in c["configurations"]
                )
            )


if __name__ == "__main__":
    unittest.main()
