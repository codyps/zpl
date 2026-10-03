"""Independent geometry, interpreter, partition and reproducibility checks."""

import itertools
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import freetype

import font_probe
import reconstruct_font as pipeline
import reconstruct_geometry as geometry
import reconstruct_hints as hints
import reconstruction_probe

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


class GeometryTests(unittest.TestCase):
    def test_recovers_parabola_and_keeps_explicit_extrema(self):
        a, b, c = (0, 0), (50, 100), (100, 0)
        points = [geometry.quadratic(a, b, c, i / 100) for i in range(101)]
        fitted = geometry.fit_arc(points, 0.15)
        self.assertLessEqual(len(fitted), 7)
        split = geometry.split_extrema((*a, True), (*b, False), (*c, True))
        self.assertEqual(split[2], (50, 50, True))
        self.assertTrue(all(len(p) == 3 for p in split))
        for point in points:
            flat = geometry.flatten(split + [(100, -20, True), (0, -20, True)], 0.05)
            self.assertLess(
                min(geometry.line_error(point, p, q) for p, q in zip(flat, flat[1:])),
                0.1,
            )

    def test_ring_preserves_counter_and_uses_curves(self):
        pixels = {
            (x, y)
            for x in range(-40, 41)
            for y in range(-40, 41)
            if 20**2 <= (x + 0.5) ** 2 + (y + 0.5) ** 2 < 36**2
        }
        shape = geometry.fit_shape(pixels, 0.9, 4)
        self.assertEqual(len(shape), 2)
        self.assertTrue(any(not p[2] for contour in shape for p in contour))
        self.assertLess(sum(map(len, shape)), 150)
        loops = [geometry.flatten(c) for c in shape]
        self.assertEqual(geometry.winding((0, 0), loops), 0)
        self.assertNotEqual(geometry.winding((110, 0), loops), 0)

    def test_crossing_or_reversed_contours_are_rejected(self):
        box = [font_probe.rect(0, 0, 100, 100)]
        self.assertFalse(geometry.topology_matches(box, [list(reversed(box[0]))]))
        crossed = [[(0, 0, True), (100, 100, True), (0, 100, True), (100, 0, True)]]
        self.assertFalse(geometry.topology_matches(box, crossed))


class HintConstructionTests(unittest.TestCase):
    def shapes(self):
        ring = [
            font_probe.rect(0, 75, 400, 600),
            list(reversed(font_probe.rect(40, 115, 320, 520))),
        ]
        return {"A": ring, "B": ring}

    def test_infers_stems_and_zones_without_character_constants(self):
        inferred = hints.infer(self.shapes())
        self.assertIn(40, inferred["shared_widths"][0])
        self.assertNotIn(320, inferred["shared_widths"][0])
        self.assertTrue(inferred["zones"])
        self.assertNotIn(0, inferred["zones"])

    def test_every_policy_executes_in_independent_interpreter(self):
        shapes = self.shapes()
        inferred = hints.infer(shapes)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "font.ttf"
            for policy in itertools.product(hints.POLICIES, repeat=2):
                font = hints.build(shapes, inferred, {c: policy for c in shapes})
                self.assertEqual(font_probe.checksum(font), 0xB1B0AFBA)
                path.write_bytes(font)
                face = freetype.Face(str(path))
                for x, y in ((11, 11), (13, 23), (23, 13), (40, 40)):
                    face.set_pixel_sizes(x, y)
                    face.load_char(
                        "A",
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC,
                    )
                    self.assertEqual(len(face.glyph.outline.contours), 2)
                    self.assertEqual(face.glyph.advance.x, x * 64)

    def test_hint_range_preserves_large_and_stretched_geometry(self):
        shapes = self.shapes()
        inferred = hints.infer(shapes)
        inferred["hint_ppem_limit"] = 90
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            faces = []
            for label, policy in (
                ("hinted", ("grid", "grid")),
                ("plain", ("none", "none")),
            ):
                path = root / (label + ".ttf")
                path.write_bytes(
                    hints.build(shapes, inferred, {c: policy for c in shapes})
                )
                faces.append(freetype.Face(str(path)))
            for x, y in ((91, 91), (256, 256), (120, 13), (13, 120)):
                points = []
                for face in faces:
                    face.set_pixel_sizes(x, y)
                    face.load_char(
                        "A",
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC,
                    )
                    points.append(face.glyph.outline.points)
                self.assertEqual(*points)


class PipelineTests(unittest.TestCase):
    def test_frozen_font_and_printer_requests_reproduce(self):
        root = ROOT / "reconstruction-20261003"
        model, _ = pipeline.frozen(root)
        self.assertEqual(sum(sum(map(len, s)) for s in model["shapes"].values()), 632)
        self.assertFalse(model["hint_decisions"]["@"]["accepted"])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "prepared"
            pipeline.prepare_preview(
                root,
                [ROOT / "reconstruction-validation-20261003"],
                output,
                "R:ZP26G.TTF",
            )
            for path in output.iterdir():
                self.assertEqual(
                    path.read_bytes(),
                    (ROOT / "reconstruction-replay-20261003" / path.name).read_bytes(),
                )

    def test_frozen_font_executes_curves_and_hint_range_independently(self):
        root = ROOT / "reconstruction-20261003"
        hinted = freetype.Face(str(root / "font.ttf"))
        plain = freetype.Face(str(root / "geometry.ttf"))
        for x, y in (
            (11, 12),
            (12, 11),
            (13, 13),
            (37, 37),
            (49, 49),
            (90, 90),
            (91, 91),
            (320, 448),
        ):
            for char in "HOSgj@":
                outlines = []
                for face in (hinted, plain):
                    face.set_pixel_sizes(x, y)
                    face.load_char(
                        char,
                        freetype.FT_LOAD_TARGET_MONO
                        | freetype.FT_LOAD_NO_AUTOHINT
                        | freetype.FT_LOAD_PEDANTIC,
                    )
                    self.assertGreater(len(face.glyph.outline.points), 0)
                    outlines.append(face.glyph.outline.points)
                if max(x, y) > 90:
                    self.assertEqual(*outlines)

    def test_tampered_frozen_artifact_is_rejected(self):
        import shutil

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "run"
            shutil.copytree(ROOT / "reconstruction-20261003", output)
            with (output / "font.ttf").open("ab") as file:
                file.write(b"changed")
            with self.assertRaisesRegex(ValueError, "frozen font changed"):
                pipeline.frozen(output)

    def test_reserved_probe_reproduces_and_excludes_prior_configurations(self):
        fixture = ROOT / "reconstruction-validation-20261003"
        manifest = json.loads((fixture / "manifest.json").read_text())
        configurations = [tuple(q) for q in manifest["configurations"]]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "prepared"
            reconstruction_probe.prepare(
                output, manifest["characters"], configurations, "validation"
            )
            for path in output.iterdir():
                self.assertEqual(path.read_bytes(), (fixture / path.name).read_bytes())
        for previous in ("small-font-20261003", "small-font-validation-20261003"):
            old = json.loads((ROOT / previous / "manifest.json").read_text())
            keys = {
                (p["width"], p["height"], p["orientation"])
                for page in old["pages"]
                if page["font"] == "0"
                for p in page["probes"]
            }
            self.assertFalse(keys & set(configurations))

    def test_fit_loader_does_not_open_validation_pages(self):
        root = ROOT / "small-font-20261003"
        manifest = json.loads((root / "manifest.json").read_text())
        forbidden = {
            root / "zd621" / (p["name"] + ".png")
            for p in manifest["pages"]
            if p["group"] == "validation"
        }
        real = Path.read_bytes

        def checked(path):
            self.assertNotIn(path, forbidden)
            return real(path)

        with mock.patch.object(Path, "read_bytes", checked):
            pages, _ = pipeline.load_pages(root, "development")
        self.assertEqual(len(pipeline.cases(pages)), 90)

    def test_content_cache_and_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "engine"
            binary.write_bytes(b"identity")
            engine = pipeline.Engine(binary, root / "cache", 1)
            query = (10, 10, 65, 0)
            rows = {query: {"width": 0, "height": 0, "contours": []}}
            with mock.patch.object(
                pipeline.engine_api, "engine_rows", return_value=rows
            ) as render:
                engine.render(b"font", [query])
                engine.render(b"font", [query])
                self.assertEqual(render.call_count, 1)
                with self.assertRaisesRegex(ValueError, "budget exhausted"):
                    engine.render(b"other", [query])
            resumed = pipeline.Engine(binary, root / "cache", 2)
            with mock.patch.object(
                pipeline.engine_api,
                "engine_rows",
                side_effect=AssertionError("cache miss"),
            ):
                self.assertNotIn("contours", resumed.render(b"font", [query])[query])


if __name__ == "__main__":
    unittest.main()
