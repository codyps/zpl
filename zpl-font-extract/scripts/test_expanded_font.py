"""Expanded sampling separation and exact-score reuse contracts."""

from copy import deepcopy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

import expanded_font_probe as sampling
import compare_font_fits
import joint_hint_program as hints
import optimize_font as optimizer
import reconstruct_font as pipeline

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"
PREVIOUS = [
    ROOT / name
    for name in (
        "font0-outline-20261003",
        "small-font-20261003",
        "small-font-validation-20261003",
        "reconstruction-validation-20261003",
        "joint-validation-20261003",
        "joint-large-validation-v2-20261003",
    )
]


class ExpandedTests(unittest.TestCase):
    def test_expanded_replay_reproduces_and_programs_execute_independently(self):
        root = ROOT / "expanded-optimization-20261004"
        model, _ = pipeline.frozen(root)
        plan_root = ROOT / "expanded-font-20261003"
        plan = json.loads((plan_root / "plan.json").read_text())
        captures = [
            plan_root / c["name"]
            for c in plan["campaigns"]
            if c["group"] == "validation" and c["role"] == "small"
        ]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "replay"
            pipeline.prepare_preview(
                root, captures, output, "R:ZP26I.TTF", variant="proposal"
            )
            for path in output.iterdir():
                self.assertEqual(
                    path.read_bytes(),
                    (ROOT / "expanded-replay-20261004" / path.name).read_bytes(),
                )
        face = freetype.Face(str(root / "font.ttf"))
        for x, y in (
            (5, 10),
            (8, 8),
            (29, 28),
            (64, 48),
            (80, 80),
            (90, 91),
            (96, 192),
            (400, 400),
        ):
            face.set_pixel_sizes(x, y)
            for char in model["shapes"]:
                face.load_char(
                    char,
                    freetype.FT_LOAD_TARGET_MONO
                    | freetype.FT_LOAD_NO_AUTOHINT
                    | freetype.FT_LOAD_PEDANTIC,
                )
                self.assertGreater(len(face.glyph.outline.points), 0)

    def test_comparison_rejects_unaccounted_native_pixels(self):
        path = ROOT / "joint-optimization-20261003/evaluation.json"
        old = json.loads(path.read_text())
        changed = deepcopy(old)
        changed["campaigns"][0]["fonts"]["proposal.ttf"][0]["under"] += 1
        with self.assertRaisesRegex(ValueError, "native canvas"):
            compare_font_fits.compare(old, changed)

    def test_expanded_model_keeps_both_old_and_new_checks_out_of_search(self):
        model, _ = pipeline.frozen(ROOT / "expanded-optimization-20261004")
        old, _ = pipeline.frozen(ROOT / "joint-optimization-20261003")
        search = set(map(tuple, model["search_queries"]))
        internal = set(map(tuple, model["internal_check_queries"]))
        self.assertEqual((len(search), len(internal)), (894, 426))
        self.assertFalse(search & internal)
        self.assertFalse(search & set(map(tuple, old["internal_check_queries"])))
        plan = json.loads((ROOT / "expanded-font-20261003/plan.json").read_text())
        validation = {
            (w, h, ord(c), "NRIB".index(r))
            for campaign in plan["campaigns"]
            if campaign["group"] == "validation"
            for row in campaign["configurations"]
            for w, h, r in [row["query"]]
            for c in "HOSgj@"
        }
        self.assertEqual(len(validation), 324)
        self.assertFalse(validation & (search | internal))
        artifact = "proposal.ttf" if model["acceptance"]["accepted"] else "initial.ttf"
        root = ROOT / "expanded-optimization-20261004"
        self.assertEqual(
            (root / "font.ttf").read_bytes(), (root / artifact).read_bytes()
        )

    def test_plan_reproduces_without_opening_any_pixels(self):
        def no_pixels(path):
            self.assertNotEqual(path.suffix, ".png")
            return original(path)

        original = Path.read_bytes
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(
            Path, "read_bytes", no_pixels
        ):
            root = Path(directory) / "plan"
            sampling.prepare(root, PREVIOUS)
            for path in root.rglob("*"):
                if path.is_file():
                    self.assertEqual(
                        path.read_bytes(),
                        (
                            ROOT / "expanded-font-20261003" / path.relative_to(root)
                        ).read_bytes(),
                    )
            plan = json.loads((root / "plan.json").read_text())
            self.assertEqual(plan["cases"], 1536)
            self.assertTrue(all(c["previews"] <= 32 for c in plan["campaigns"]))

    def test_new_development_never_reuses_prior_queries(self):
        rows, _ = sampling.configurations(PREVIOUS)
        old = set()
        for root in PREVIOUS:
            for page in json.loads((root / "manifest.json").read_text())["pages"]:
                if page.get("font", "0") == "0":
                    old.update(
                        (p["width"] or p["height"], p["height"], p["orientation"])
                        for p in page["probes"]
                    )
        new = {tuple(r["query"]) for r in rows}
        self.assertEqual(len(rows), len(new))
        self.assertFalse(old & new)
        self.assertEqual({r["query"][2] for r in rows}, set("NRIB"))

    def test_additional_loader_rejects_overlap(self):
        root = ROOT / "small-font-20261003"
        pages, provenance = pipeline.load_pages(root, "development")
        refs = pipeline.cases(pages)
        with self.assertRaisesRegex(ValueError, "reserved query"):
            optimizer.additional_data(
                [root], provenance["printer"], "HOSgj@", {next(iter(refs))}
            )

    def test_hint_change_reuses_only_the_guarded_outline_scores(self):
        model, _ = pipeline.frozen(ROOT / "reconstruction-20261003")
        state = hints.initialize(model)
        state["shapes"] = {"H": state["shapes"]["H"]}
        small, edge, large = (12, 12, 72, 0), (90, 90, 72, 0), (256, 256, 72, 0)
        refs = {q: {(0, 0)} for q in (small, edge, large)}
        fake = mock.Mock()
        fake.render.side_effect = lambda data, keys: {
            q: {"pixels": {(0, 0)}} for q in keys
        }
        with mock.patch.object(
            pipeline, "pixels", side_effect=lambda row: row["pixels"]
        ):
            oracle = optimizer.Oracle(fake, refs, state)
            candidate = deepcopy(state)
            candidate["programs"]["H"][0][0] = dict(op="anchor", round="half")
            oracle.errors(candidate)
        self.assertEqual(fake.render.call_count, 3)
        self.assertEqual(fake.render.call_args_list[-1].args[1], [small, edge])


if __name__ == "__main__":
    unittest.main()
