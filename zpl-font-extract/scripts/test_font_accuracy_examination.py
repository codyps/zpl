"""Evidence accounting and additive curve-feature experiments."""

from copy import deepcopy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

import evaluate_font_experiment as audit
import examine_font_accuracy as examination
import font_probe
import joint_hint_program as hints
import reconstruct_font as pipeline
import refine_font_features as features

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


class AccuracyExaminationTests(unittest.TestCase):
    def test_frozen_experiments_recompile_and_execute_in_independent_interpreter(self):
        root = ROOT / "accuracy-examination-20261004"
        candidates = json.loads((root / "ablations/candidates.json").read_text())[
            "states"
        ]
        candidates["extended_features"] = json.loads(
            (root / "features/candidate.json").read_text()
        )
        for label, state in candidates.items():
            path = root / (
                "features/candidate.ttf"
                if label == "extended_features"
                else f"ablations/{label}.ttf"
            )
            self.assertEqual(hints.build(state), path.read_bytes())
            face = freetype.Face(str(path))
            for x, y in (
                (10, 10),
                (15, 13),
                (31, 27),
                (47, 53),
                (83, 71),
                (90, 91),
                (432, 432),
            ):
                face.set_pixel_sizes(x, y)
                for char in state["shapes"]:
                    face.load_char(
                        char,
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC,
                    )
                    self.assertGreater(len(face.glyph.outline.points), 0)
        plan = json.loads((root / "audit-plan.json").read_text())
        for artifact in plan["frozen_artifacts"]:
            self.assertEqual(
                pipeline.sha((ROOT / artifact["path"]).read_bytes()), artifact["sha256"]
            )
        replay = ROOT / "accuracy-feature-replay-20261004"
        self.assertEqual(
            hints.build(candidates["extended_features"], witnesses=True),
            (replay / "probe.ttf").read_bytes(),
        )

    def test_audit_rejects_reused_fitting_cases_before_rendering(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "engine").write_bytes(b"test-engine")
            (root / "font.ttf").write_bytes(b"frozen-font")
            (root / "capture/zd621").mkdir(parents=True)
            (root / "capture/manifest.json").write_text("{}")
            (root / "capture/zd621/capture.json").write_text(
                json.dumps(dict(started_utc="2026-10-04T12:00:00+00:00"))
            )
            query = (11, 9, 72, 0)
            plan = dict(
                schema="font-experiment-audit-v1",
                engine_sha256=pipeline.sha(b"test-engine"),
                frozen_at="2026-10-04T11:00:00+00:00",
                frozen_artifacts=[
                    dict(path="font.ttf", sha256=pipeline.sha(b"frozen-font"))
                ],
                fitting_queries=[query],
                captures=[dict(path="capture", manifest_sha256=pipeline.sha(b"{}"))],
                printer={"model": "ZD621"},
            )
            pipeline.save(root / "plan.json", plan)
            with mock.patch.object(
                pipeline, "load_pages", return_value=([], dict(printer=plan["printer"]))
            ), mock.patch.object(
                pipeline, "cases", return_value={query: set()}
            ), mock.patch.object(
                pipeline, "Engine"
            ) as engine:
                with self.assertRaisesRegex(ValueError, "fitting or duplicate"):
                    audit.evaluate(
                        root / "plan.json",
                        root,
                        root / "engine",
                        root / "cache",
                        root / "output.json",
                    )
                engine.assert_not_called()
            (root / "font.ttf").write_bytes(b"changed-font")
            with self.assertRaisesRegex(ValueError, "frozen experiment changed"):
                audit.evaluate(
                    root / "plan.json",
                    root,
                    root / "engine",
                    root / "cache",
                    root / "output.json",
                )

    def test_local_extrema_add_missing_s_horizontal_stems_without_changing_font(self):
        model, _ = pipeline.frozen(ROOT / "expanded-optimization-20261004")
        state = model["state"]
        before = deepcopy(state)
        candidate, additions = features.extend(state)
        self.assertEqual(state, before)
        self.assertEqual(candidate["shapes"], state["shapes"])
        self.assertEqual(hints.build(candidate), hints.build(state))
        self.assertEqual(state["graph"]["S"][1]["stems"], [])
        added = additions["S"][1]
        self.assertEqual([g["value"] for g in added["groups"]], [208, 1323])
        self.assertEqual([p["width"] for p in added["stems"]], [240, 240])
        for char, axes in candidate["programs"].items():
            for axis, program in enumerate(axes):
                hints.ordered(program, candidate["graph"][char][axis])

    def test_inner_counter_cannot_become_an_ink_stem(self):
        outer = font_probe.rect(0, 0, 1000, 1000)
        inner = list(reversed(font_probe.rect(300, 300, 400, 400)))
        state = dict(
            shapes={"O": [outer, inner]},
            graph={"O": [dict(groups=[], stems=[]), dict(groups=[], stems=[])]},
            programs={"O": [[], []]},
            parameters=dict(widths=[[300], [300]], zones=[], cutin=16, limit=90),
            zone_origins=[],
        )
        candidate, _ = features.extend(state)
        for feature in candidate["graph"]["O"]:
            self.assertEqual(len(feature["stems"]), 2)
            self.assertEqual({p["width"] for p in feature["stems"]}, {300})

    def test_native_counts_must_account_for_background_foreground(self):
        case = dict(
            text="H",
            width=10,
            height=10,
            orientation="N",
            under=1,
            over=2,
            xor=3,
            union=10,
        )
        page = dict(cases=[case], under=1, over=2, xor=3, union=10)
        self.assertEqual(examination.report_cases([page]), [case])
        page["over"] += 1
        with self.assertRaisesRegex(ValueError, "native canvas"):
            examination.report_cases([page])

    def test_breakdown_handles_blank_exact_cases_and_missing_strata(self):
        blank = dict(
            text="H",
            width=0,
            height=10,
            orientation="N",
            under=0,
            over=0,
            xor=0,
            union=0,
        )
        result = examination.breakdown([blank])
        self.assertEqual(result["total"]["iou"], 1)
        self.assertEqual(result["total"]["exact"], 1)
        self.assertEqual(set(result["sizes"]), {"up_to_32"})
        self.assertEqual(set(result["transforms"]), {"square"})


if __name__ == "__main__":
    unittest.main()
