"""Offline checks for generated hint programs and capture provenance."""

import json
from pathlib import Path
import tempfile
import unittest

import freetype

import font0_hint_model as hints
import font_probe
import hinted_font_probe
import compare_constructed

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


class Font0HintTests(unittest.TestCase):
    def test_stale_font_capture_cannot_be_used_as_accuracy_evidence(self):
        with self.assertRaisesRegex(ValueError, "diagnostic-only"):
            compare_constructed.compare(
                ROOT / "cvt-axis-validation-20261003", Path("unused")
            )

    def test_reproduce_uploaded_font_and_requests(self):
        fixture = ROOT / "font0-hints-20261003"
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "prepared"
            hinted_font_probe.prepare(
                output,
                ROOT / "font0-outline-20261003",
                ROOT / "small-font-20261003",
                ROOT / "small-font-validation-20261003",
                fixture / "model.json",
            )
            for file in output.iterdir():
                self.assertEqual(
                    file.read_bytes(), (fixture / file.name).read_bytes(), file.name
                )
        self.assertEqual(
            font_probe.checksum((fixture / "probe.ttf").read_bytes()), 0xB1B0AFBA
        )

    def test_independent_interpreter_accepts_frozen_hint_programs(self):
        face = freetype.Face(str(ROOT / "font0-hints-20261003/probe.ttf"))
        for x, y in [
            (10, 10),
            (11, 11),
            (13, 23),
            (23, 13),
            (19, 19),
            (25, 25),
            (32, 32),
            (40, 40),
        ]:
            face.set_pixel_sizes(x, y)
            for c in "HOSgj@":
                face.load_char(
                    c,
                    freetype.FT_LOAD_TARGET_MONO
                    | freetype.FT_LOAD_NO_AUTOHINT
                    | freetype.FT_LOAD_PEDANTIC,
                )
                self.assertGreater(len(face.glyph.outline.points), 0)
                self.assertEqual(face.glyph.advance.x, x * 64)

    def test_cycles_and_geometry_mismatches_are_rejected(self):
        shape = [font_probe.rect(0, 0, 296, 1532)]
        params = json.loads((ROOT / "font0-hints-20261003/model.json").read_text())[
            "parameters"
        ]
        with self.assertRaisesRegex(ValueError, "cyclic"):
            hints.compile_hints(
                {"H": shape}, {"H": [["next", "prev"], ["none", "none"]]}, params
            )
        with self.assertRaisesRegex(ValueError, "group count"):
            hints.compile_hints(
                {"H": shape}, {"H": [["grid"], ["none", "none"]]}, params
            )

    def test_final_reservation_is_disjoint_from_refinement_sizes(self):
        original = json.loads((ROOT / "small-font-20261003/manifest.json").read_text())
        final = json.loads(
            (ROOT / "small-font-validation-20261003/manifest.json").read_text()
        )
        configurations = lambda m: {
            (p["width"], p["height"], p["orientation"])
            for page in m["pages"]
            if page["font"] == "0"
            for p in page["probes"]
        }
        self.assertFalse(configurations(original) & configurations(final))
        self.assertEqual(sum(len(p["probes"]) for p in final["pages"]), 90)
        self.assertTrue(all(p["group"] == "validation" for p in final["pages"]))


if __name__ == "__main__":
    unittest.main()
