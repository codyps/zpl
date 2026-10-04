"""Independent interpreter and optimization-contract checks."""

from copy import deepcopy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

import font_probe
import joint_font_geometry as geometry
import joint_hint_program as hints
import optimize_font as optimizer
import reconstruct_font as pipeline
import reconstruct_hints
import reconstruction_probe

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


def seed():
    return pipeline.frozen(ROOT / "reconstruction-20261003")[0]


def points(face, char, x, y):
    face.set_pixel_sizes(x, y)
    face.load_char(
        char,
        freetype.FT_LOAD_TARGET_MONO
        | freetype.FT_LOAD_NO_AUTOHINT
        | freetype.FT_LOAD_PEDANTIC,
    )
    return face.glyph.outline.points


class HintTests(unittest.TestCase):
    def test_migration_preserves_legacy_in_independent_interpreter(self):
        model = seed()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "joint.ttf"
            path.write_bytes(hints.build(hints.initialize(model)))
            new = freetype.Face(str(path))
            old = freetype.Face(str(ROOT / "reconstruction-20261003/font.ttf"))
            for char in model["shapes"]:
                for x, y in (
                    (11, 11),
                    (17, 17),
                    (13, 23),
                    (23, 13),
                    (90, 90),
                    (91, 91),
                    (256, 384),
                ):
                    self.assertEqual(points(new, char, x, y), points(old, char, x, y))

    def test_neighbor_programs_execute_with_pedantic_independent_interpreter(self):
        shapes = {
            "A": [
                font_probe.rect(0, 75, 400, 600),
                list(reversed(font_probe.rect(40, 115, 320, 520))),
            ]
        }
        inferred = reconstruct_hints.infer(shapes)
        inferred["hint_ppem_limit"] = 90
        state = hints.initialize(
            dict(
                shapes=shapes,
                inferred=inferred,
                cutin=24,
                policies={"A": ["stems-low", "zones"]},
            )
        )
        observed = set()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "font.ttf"
            for program in hints.neighbors(state, "A"):
                candidate = deepcopy(state)
                candidate["programs"]["A"] = program
                try:
                    data = hints.build(candidate)
                except ValueError as error:
                    self.assertIn(
                        str(error),
                        (
                            "cyclic hint references",
                            "hint link requires a touched anchor",
                        ),
                    )
                    continue
                self.assertEqual(font_probe.checksum(data), 0xB1B0AFBA)
                path.write_bytes(data)
                face = freetype.Face(str(path))
                for x, y in ((11, 11), (13, 23), (23, 13)):
                    self.assertEqual(len(points(face, "A", x, y)), 8)
                observed.update(n["op"] for axis in program for n in axis if n)
        self.assertEqual(observed, {"anchor", "zone", "link", "center"})

    def test_cycles_and_non_stem_links_are_rejected(self):
        feature = dict(groups=[{}, {}, {}], stems=[dict(low=0, high=1)])
        nodes = [
            dict(op="link", ref=1, round="grid"),
            dict(op="link", ref=0, round="grid"),
            None,
        ]
        with self.assertRaisesRegex(ValueError, "cyclic"):
            hints.ordered(nodes, feature)
        nodes[0]["ref"] = 2
        with self.assertRaisesRegex(ValueError, "not an observed stem"):
            hints.ordered(nodes, feature)

    def test_shared_zone_parameter_actually_moves_the_outline(self):
        state = hints.initialize(seed())
        group = state["graph"]["H"][1]["groups"][0]
        zone = min(
            range(len(state["zone_origins"])),
            key=lambda i: abs(state["zone_origins"][i] - group["value"]),
        )
        state["programs"]["H"][1][0] = dict(op="zone", zone=zone, round="grid")
        other = deepcopy(state)
        other["parameters"]["zones"][zone] += 128
        with tempfile.TemporaryDirectory() as directory:
            outlines = []
            for i, variant in enumerate((state, other)):
                path = Path(directory) / f"{i}.ttf"
                path.write_bytes(hints.build(variant, ["H"]))
                outlines.append(points(freetype.Face(str(path)), "H", 40, 40))
            self.assertNotEqual(*outlines)


class GeometryTests(unittest.TestCase):
    def test_continuous_gradient_matches_finite_difference(self):
        field = geometry.DistanceField({(x, y) for x in range(8) for y in range(12)})
        for x, y in ((1.2, 1.8), (-0.2, 4.1), (6.8, 9.2)):
            _, dx, dy = field.sample(x, y)
            epsilon = 1e-5
            self.assertAlmostEqual(
                dx,
                (field.sample(x + epsilon, y)[0] - field.sample(x - epsilon, y)[0])
                / (2 * epsilon),
            )
            self.assertAlmostEqual(
                dy,
                (field.sample(x, y + epsilon)[0] - field.sample(x, y - epsilon)[0])
                / (2 * epsilon),
            )

    def test_edge_coordinates_are_tied_and_moves_are_bounded(self):
        shape = [font_probe.rect(0, 0, 100, 100)]
        graph = [reconstruct_hints.features(shape, a) for a in (0, 1)]
        freedoms = geometry.freedoms(shape, graph)
        self.assertEqual(sorted(len(indices) for _, indices in freedoms), [2, 2, 2, 2])
        axis, indices = freedoms[0]
        candidate = geometry.moved(shape, shape, axis, indices, 4)
        self.assertIsNotNone(candidate)
        self.assertIsNone(geometry.moved(candidate, shape, axis, indices, 9))
        self.assertEqual(shape[0][0][0], 0)


class SearchTests(unittest.TestCase):
    def test_frozen_joint_font_and_printer_campaign_reproduce(self):
        root = ROOT / "joint-optimization-20261003"
        model, _ = pipeline.frozen(root)
        self.assertFalse(model["acceptance"]["accepted"])
        self.assertEqual(
            (root / "font.ttf").read_bytes(), (root / "initial.ttf").read_bytes()
        )
        self.assertLess(
            model["training"]["proposal_objective"],
            model["training"]["before_objective"],
        )
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "prepared"
            pipeline.prepare_preview(
                root,
                [ROOT / "joint-validation-20261003"],
                output,
                "R:ZP26H.TTF",
                variant="proposal",
            )
            for path in output.iterdir():
                self.assertEqual(
                    path.read_bytes(),
                    (ROOT / "joint-replay-20261003" / path.name).read_bytes(),
                )
        face = freetype.Face(str(root / "proposal.ttf"))
        unhinted = deepcopy(model["proposal"])
        unhinted["programs"] = {
            c: [[None] * len(nodes) for nodes in axes]
            for c, axes in unhinted["programs"].items()
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plain.ttf"
            path.write_bytes(hints.build(unhinted))
            plain = freetype.Face(str(path))
            for char in model["shapes"]:
                for x, y in (
                    (10, 11),
                    (13, 12),
                    (42, 42),
                    (52, 52),
                    (91, 91),
                    (288, 416),
                ):
                    outline = points(face, char, x, y)
                    self.assertGreater(len(outline), 0)
                    if max(x, y) > 90:
                        self.assertEqual(outline, points(plain, char, x, y))

    def test_new_capture_plans_reproduce_and_exclude_seed_queries(self):
        training = {tuple(q) for q in seed()["training_queries"]}
        for name in ("joint-validation-20261003", "joint-large-validation-v2-20261003"):
            fixture = ROOT / name
            manifest = json.loads((fixture / "manifest.json").read_text())
            queries = {
                (w, h, ord(c), "NRIB".index(r))
                for w, h, r in manifest["configurations"]
                for c in manifest["characters"]
            }
            self.assertFalse(queries & training)
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "plan"
                reconstruction_probe.prepare(
                    output,
                    manifest["characters"],
                    [tuple(q) for q in manifest["configurations"]],
                    "validation",
                )
                for path in output.iterdir():
                    self.assertEqual(
                        path.read_bytes(), (fixture / path.name).read_bytes()
                    )

    def test_large_plan_fits_descenders_and_rejects_clipped_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "plan"
            reconstruction_probe.prepare(root, "gj", [(512, 512, "N")], "validation")
            plan = json.loads((root / "manifest.json").read_text())
            for page in plan["pages"]:
                for probe in page["probes"]:
                    self.assertLessEqual(
                        probe["tile"][0] + probe["tile"][2], page["canvas"][0]
                    )
                    self.assertGreater(probe["tile"][3] - probe["anchor"][1], 96)
        with self.assertRaisesRegex(ValueError, "diagnostic-only"):
            pipeline.load_pages(ROOT / "joint-large-validation-20261003", "validation")

    def test_exact_oracle_rejects_a_large_sample_regression(self):
        baseline = hints.initialize(seed())
        baseline["shapes"] = {"H": baseline["shapes"]["H"]}
        small, large = (12, 12, ord("H"), 0), (256, 256, ord("H"), 0)
        references = {small: {(0, 0)}, large: {(0, 0)}}
        fake = mock.Mock()
        old = {small: {"pixels": set()}, large: {"pixels": {(0, 0)}}}
        new = {small: {"pixels": {(0, 0)}}, large: {"pixels": set()}}
        baseline_font = hints.build(baseline, ["H"])
        fake.render.side_effect = lambda data, keys: {
            q: (old if data == baseline_font else new)[q] for q in keys
        }
        with mock.patch.object(
            pipeline, "pixels", side_effect=lambda row: row["pixels"]
        ):
            oracle = optimizer.Oracle(fake, references, baseline)
            candidate = deepcopy(baseline)
            candidate["shapes"]["H"][0][0][0] += 1
            self.assertEqual(oracle.score(candidate), float("inf"))
            self.assertLess(oracle.score(baseline), float("inf"))
        self.assertEqual(fake.render.call_count, 4)

    def test_partition_reserves_whole_size_configurations(self):
        small = {(x, x, c, 0): set() for x in range(10, 19) for c in (65, 66)}
        train, check = optimizer.partition({(256, 256, 65, 0): set()}, small)
        self.assertFalse(set(train) & set(check))
        self.assertEqual({q[0] for q in check}, {11, 14, 17})
        self.assertFalse(
            {(q[0], q[1], q[3]) for q in train} & {(q[0], q[1], q[3]) for q in check}
        )

    def test_atomic_acceptance_cannot_hide_transformed_regression(self):
        before = {(11, 11, 65, 0): 0.2, (12, 14, 65, 0): 0.3}
        self.assertFalse(
            optimizer.accepts(before, {(11, 11, 65, 0): 0.0, (12, 14, 65, 0): 0.31})
        )
        self.assertTrue(
            optimizer.accepts(before, {(11, 11, 65, 0): 0.1, (12, 14, 65, 0): 0.3})
        )

    def test_beam_can_cross_a_worse_intermediate_program(self):
        state = dict(programs={"A": [0]}, graph={"A": [{}]})
        scores = {0: 5, 1: 6, 2: 7, 3: 1}

        class Oracle:
            def score(self, candidate):
                return scores[candidate["programs"]["A"][0]]

        def neighbors(candidate, char):
            return {0: [[1], [2]], 1: [[3]], 2: [], 3: []}[
                candidate["programs"][char][0]
            ]

        with mock.patch.object(
            hints, "neighbors", side_effect=neighbors
        ), mock.patch.object(hints, "ordered"):
            result = optimizer.beam(state, "A", Oracle(), 2, 2, 64, 0)
        self.assertEqual(result["programs"]["A"], [3])


if __name__ == "__main__":
    unittest.main()
