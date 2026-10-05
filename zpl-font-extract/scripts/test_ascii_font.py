"""Printable coverage, sparse cmap witnesses, and measured spacing contracts."""

from copy import deepcopy
import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

import ascii_font_metrics as metrics
import ascii_font_probe as probe
import ascii_cmap_probe
import font_probe
import joint_hint_program as hints
import reconstruct_font as pipeline
import reconstruct_hints
import font_experiment_replay
from fit_ascii_hints import one_glyph, replace_glyph
from fit_font_target import metrics as target_metrics
from fit_structured_hints import Objective
from merge_ascii_font import merge
from audit_ascii_font import text_pages
from test_structured_hints import specimen


class AsciiTests(unittest.TestCase):
    def test_full_font_replays_reproduce_and_share_the_transport_page_budget(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        fit = fixtures / "ascii-reconstruction-20261004"
        with tempfile.TemporaryDirectory() as directory:
            for variant, name, object_name in (
                ("baseline", "ascii-baseline-replay-20261004", "R:ZP26P.TTF"),
                ("targeted", "ascii-candidate-replay-20261004", "R:ZP26Q.TTF"),
            ):
                output = Path(directory) / name
                font_experiment_replay.prepare(
                    fit,
                    variant,
                    fixtures,
                    [fixtures / "ascii-font-20261004/validation-hints-07"],
                    output,
                    object_name,
                )
                for path in output.iterdir():
                    self.assertEqual(
                        path.read_bytes(), (fixtures / name / path.name).read_bytes()
                    )
                self.assertGreater((output / "probe.ttf").stat().st_size, 65536)
            output = Path(directory) / "excessive"
            with self.assertRaisesRegex(ValueError, "preview budget"):
                font_experiment_replay.prepare(
                    fit,
                    "targeted",
                    fixtures,
                    [fixtures / "ascii-font-20261004/validation-outline-08"],
                    output,
                    "R:ZP26T.TTF",
                )
            self.assertFalse(output.exists())

    def test_text_audit_measures_space_and_rejects_unaccounted_native_pixels(self):
        probe = dict(
            text="A A",
            width=16,
            height=16,
            orientation="N",
            origin="FT",
            tile=[0, 0, 16, 8],
            anchor=[2, 3],
        )
        page = dict(name="spacing", canvas=[16, 8], probes=[probe])
        rows = {
            (16, 16, 65, 0): dict(
                left=0, top=-1, width=1, rows=["80"], layout_advance=3
            ),
            (16, 16, 32, 0): dict(left=0, top=0, width=0, rows=[], layout_advance=2),
        }
        reference = {(2, 2), (7, 2)}
        self.assertEqual(text_pages([(page, reference)], rows)[0]["xor"], 0)
        rows[(16, 16, 32, 0)]["layout_advance"] = 3
        score = text_pages([(page, reference)], rows)[0]
        self.assertEqual((score["under"], score["over"], score["union"]), (1, 1, 3))
        with self.assertRaisesRegex(ValueError, "unaccounted reference"):
            text_pages([(page, reference | {(20, 2)})], rows)
        rows[(16, 16, 32, 0)]["layout_advance"] = 20
        with self.assertRaisesRegex(ValueError, "escapes native field"):
            text_pages([(page, reference)], rows)

    def test_frozen_fonts_and_training_constraints_reproduce(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        target = fixtures / "target-hints-20261004"
        report = json.loads((target / "report.json").read_text())
        states = json.loads((target / "candidates.json").read_text())
        plan = json.loads((target / "plan.json").read_text())
        training = set(map(tuple, plan["search_queries"]))
        scores = {}
        for label, state in states.items():
            data = hints.build(state)
            self.assertEqual(data, (target / (label + ".ttf")).read_bytes())
            self.assertEqual(probe.sha(data), report["variants"][label]["font_sha256"])
            cases = {
                (
                    c["width"] or c["height"],
                    c["height"],
                    ord(c["text"]),
                    "NRIB".index(c["orientation"]),
                ): c
                for page in report["variants"][label]["pages"]
                for c in page["cases"]
            }
            scores[label] = {q: cases[q] for q in training}
        self.assertEqual(
            Objective(scores["baseline"], "robust").measure(scores["targeted"])[1], []
        )
        self.assertEqual(target_metrics(scores["targeted"])["at_least_90"], 767)
        self.assertFalse(target_metrics(scores["targeted"])["passes_target"])
        bootstrap = fixtures / "ascii-bootstrap-20261004"
        state = json.loads((bootstrap / "state.json").read_text())
        self.assertEqual(hints.build(state), (bootstrap / "font.ttf").read_bytes())
        self.assertEqual(set(state["shapes"]), set(probe.NEW_GLYPHS))
        self.assertEqual(set(state["advances"]), set(probe.PRINTABLE))
        self.assertEqual(
            set(merge(states["targeted"], state)["shapes"]), set(probe.VISIBLE)
        )
        complete = fixtures / "ascii-reconstruction-20261004"
        assembled = json.loads((complete / "candidates.json").read_text())
        fitted_ascii = json.loads(
            (fixtures / "ascii-hints-20261004/state.json").read_text()
        )
        for label, part in (("baseline", state), ("targeted", fitted_ascii)):
            self.assertEqual(merge(states[label], part), assembled[label])
            self.assertEqual(
                hints.build(assembled[label]),
                (complete / (label + ".ttf")).read_bytes(),
            )
            self.assertEqual(set(assembled[label]["shapes"]), set(probe.VISIBLE))
            self.assertEqual(set(assembled[label]["advances"]), set(probe.PRINTABLE))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "cmap"
            ascii_cmap_probe.prepare(root)
            for path in root.iterdir():
                self.assertEqual(
                    path.read_bytes(),
                    (fixtures / "ascii-cmap-20261004" / path.name).read_bytes(),
                )

    def test_frozen_ascii_plan_reproduces_from_metadata_without_reading_pixels(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        frozen = fixtures / "ascii-font-20261004"
        original = Path.read_bytes

        def metadata_only(path):
            self.assertNotEqual(path.suffix, ".png")
            return original(path)

        with tempfile.TemporaryDirectory() as directory, mock.patch.object(
            Path, "read_bytes", metadata_only
        ):
            output = Path(directory) / "planned"
            probe.prepare(output, fixtures, frozen / "plan.json")
            for path in output.rglob("*"):
                if path.is_file():
                    self.assertEqual(
                        path.read_bytes(),
                        (frozen / path.relative_to(output)).read_bytes(),
                    )

    def test_merge_preserves_both_cvts_and_glyph_programs_in_independent_interpreter(
        self,
    ):
        root = Path(__file__).resolve().parents[1] / "tests/fixtures"
        first = pipeline.frozen(root / "expanded-optimization-20261004")[0]["state"]
        shapes = {"!": [font_probe.rect(30, 20, 180, 1200)]}
        inferred = reconstruct_hints.infer(shapes)
        inferred["shared_widths"] = [[212], []]
        inferred["hint_ppem_limit"] = first["parameters"]["limit"]
        second = hints.initialize(
            dict(
                shapes=shapes,
                inferred=inferred,
                cutin=24,
                policies={"!": ["stems-center", "zones"]},
            )
        )
        second["advances"] = {"!": 570, " ": 576}
        combined = merge(first, second)
        self.assertEqual(combined["advances"], second["advances"])
        with tempfile.TemporaryDirectory() as directory:
            faces = []
            for index, state in enumerate((first, second, combined)):
                path = Path(directory) / f"font-{index}.ttf"
                path.write_bytes(hints.build(state))
                faces.append(freetype.Face(str(path)))
            for size in ((10, 14), (16, 16), (21, 21), (32, 48), (96, 192), (384, 384)):
                for face in faces:
                    face.set_pixel_sizes(*size)
                for index, state in enumerate((first, second)):
                    for char in state["shapes"]:
                        flags = freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_PEDANTIC
                        faces[index].load_char(char, flags)
                        faces[2].load_char(char, flags)
                        self.assertEqual(
                            faces[index].glyph.outline.points,
                            faces[2].glyph.outline.points,
                        )
        restored = deepcopy(first)
        for char in first["shapes"]:
            replace_glyph(restored, one_glyph(first, char), char)
        self.assertEqual(hints.build(first), hints.build(restored))

    def test_plan_covers_ascii_and_reserves_disjoint_validation_before_pixels(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "plan"
            probe.prepare(root, fixtures)
            plan = json.loads((root / "plan.json").read_text())
            self.assertEqual(plan["characters"], probe.PRINTABLE)
            queries = {"development": set(), "validation": set()}
            spacing = {"development": set(), "validation": set()}
            for c in plan["campaigns"]:
                self.assertLessEqual(c["previews"], 32)
                m = json.loads((root / c["name"] / "manifest.json").read_text())
                self.assertEqual(
                    c["manifest_sha256"],
                    probe.sha((root / c["name"] / "manifest.json").read_bytes()),
                )
                for page in m["pages"]:
                    self.assertLessEqual(page["canvas"][0], 768)
                    self.assertLessEqual(page["canvas"][1], 1536)
                    for p in page["probes"]:
                        if c["role"] == "spacing":
                            if p["character"] is not None:
                                spacing[c["group"]].add((p["character"], p["height"]))
                        else:
                            q = (p["text"], p["width"], p["height"], p["orientation"])
                            self.assertNotIn(q, queries[c["group"]])
                            queries[c["group"]].add(q)
            self.assertFalse(queries["development"] & queries["validation"])
            self.assertFalse(spacing["development"] & spacing["validation"])
            self.assertEqual(
                {q[0] for q in queries["development"]}, set(probe.NEW_GLYPHS)
            )
            self.assertEqual({q[0] for q in queries["validation"]}, set(probe.VISIBLE))
            for group in spacing:
                self.assertEqual({q[0] for q in spacing[group]}, set(probe.PRINTABLE))

    def test_full_ascii_witnesses_preserve_punctuation_in_independent_interpreter(self):
        state = specimen()
        for c in probe.VISIBLE:
            if c != "H":
                for key in ("shapes", "graph", "programs"):
                    state[key][c] = deepcopy(state[key]["H"])
        state["advances"] = {c: 700 + ord(c) for c in probe.PRINTABLE}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ascii.ttf"
            path.write_bytes(hints.build(state, witnesses=True))
            face = freetype.Face(str(path))
            face.set_pixel_sizes(32, 32)
            for c in probe.VISIBLE:
                self.assertEqual(face.get_char_index(c), ord(c) - 31)
                face.load_char(c, freetype.FT_LOAD_NO_SCALE)
                self.assertEqual(face.glyph.metrics.horiAdvance, 700 + ord(c))
                self.assertTrue(face.glyph.outline.points)
            face.load_char(" ", freetype.FT_LOAD_NO_SCALE)
            self.assertEqual(face.glyph.metrics.horiAdvance, 732)
            self.assertEqual(face.glyph.outline.points, [])
            self.assertEqual(face.get_char_index(chr(0xE000)), 96)
            self.assertEqual(face.get_char_index(chr(0xE001)), 97)
            self.assertEqual(face.get_char_index(chr(0xE002)), 0)
            face.set_pixel_sizes(16, 16)
            face.load_char(
                chr(0xE001), freetype.FT_LOAD_TARGET_MONO | freetype.FT_LOAD_PEDANTIC
            )
            self.assertEqual(max(x for x, y in face.glyph.outline.points), 13 * 64)

    def test_sparse_cmap_rejects_duplicate_or_unsorted_codepoints(self):
        for codes in ((32, 32), (34, 33), (65535,)):
            with self.assertRaises(ValueError):
                font_probe.cmap4([dict(codepoint=c) for c in codes])

    def test_one_design_advance_predicts_unseen_sizes_without_a_size_table(self):
        observations = {
            (c, s): metrics.layout_advance(a, s)
            for c, a in ((" ", 576), ("W", 1812))
            for s in (12, 16, 24, 40, 80, 192)
        }
        advances, report = metrics.fit(observations)
        self.assertTrue(all(row["exact"] for row in report.values()))
        for c, a in ((" ", 576), ("W", 1812)):
            for s in (19, 37, 112):
                self.assertEqual(
                    metrics.layout_advance(advances[c], s), metrics.layout_advance(a, s)
                )
        contradictory = dict(observations)
        contradictory[" ", 192] += 8
        _, report = metrics.fit(contradictory)
        self.assertFalse(report[" "]["exact"])

    def test_midpoint_spacing_fit_uses_only_the_original_development_observations(self):
        fixtures = Path(__file__).resolve().parents[1] / "tests/fixtures"
        source = json.loads(
            (fixtures / "ascii-bootstrap-20261004/report.json").read_text()
        )
        observations = {
            (c, row["size"]): row["measured"]
            for c, record in source["spacing"].items()
            for row in record["samples"]
        }
        self.assertEqual(len(observations), 570)
        self.assertEqual({size for _, size in observations}, {12, 16, 24, 40, 80, 192})
        advances, report = metrics.fit(observations, representative="midpoint")
        for c, record in report.items():
            self.assertTrue(record["exact"])
            lower, upper = record["exact_interval"]
            # The lower midpoint wins an equal-distance integer tie.
            self.assertEqual(
                advances[c], (math.ceil(lower) + math.ceil(upper) - 1) // 2
            )
            for row in record["samples"]:
                self.assertEqual(
                    metrics.layout_advance(advances[c], row["size"]), row["measured"]
                )
        frozen = fixtures / "ascii-spacing-followup-20261004"
        fit = json.loads((frozen / "fit.json").read_text())
        self.assertEqual(report, fit["report"])
        states = json.loads((frozen / "candidates.json").read_text())
        self.assertEqual(states["midpoint"]["advances"], advances)
        expected = deepcopy(states["weighted"])
        expected["advances"] = advances
        self.assertEqual(states["midpoint"], expected)
        for label, state in states.items():
            self.assertEqual(
                hints.build(state), (frozen / (label + ".ttf")).read_bytes()
            )
        with self.assertRaisesRegex(ValueError, "unknown interval representative"):
            metrics.fit(observations, representative="unknown")

    def test_spacing_requires_unchanged_sentinels_and_accounts_for_native_ink(self):
        probes = []
        ink = set()
        for i, c in enumerate((None, " ", "A")):
            p = dict(
                text="|" + (c or "") + "|",
                character=c,
                width=16,
                height=16,
                orientation="N",
                origin="FT",
                tile=[i * 64, 0, 64, 32],
                anchor=[8, 24],
            )
            probes.append(p)
            advance = {None: 0, " ": 5, "A": 9}[c]
            points = {(x, y) for x in (0, 6 + advance) for y in range(-10, 0)}
            if c == "A":
                points |= {(x, -5) for x in range(6, 12)}
            ink |= {(x + i * 64 + 8, y + 24) for x, y in points}
        page = dict(font="0", probes=probes)
        self.assertEqual(metrics.extract([(page, ink)]), {(" ", 16): 5, ("A", 16): 9})
        with self.assertRaisesRegex(ValueError, "unaccounted"):
            metrics.extract([(page, ink | {(200, 20)})])
        ink.remove((64 + 8 + 11, 20))
        with self.assertRaisesRegex(ValueError, "terminal sentinel changed"):
            metrics.extract([(page, ink)])


if __name__ == "__main__":
    unittest.main()
