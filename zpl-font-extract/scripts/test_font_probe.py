"""Offline evidence checks; never contact a printer.

SFNT checksums: https://learn.microsoft.com/en-us/typography/opentype/spec/otff
Instruction/phantom-point semantics:
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs
The captured raster differences are baselines, not assertions of engine parity.
"""

from fractions import Fraction
import json
from pathlib import Path
import struct
import tempfile
import unittest

import freetype

import analyze_font_probe as analysis
import font_probe

FIXTURES = Path(__file__).resolve().parents[1] / "tests/fixtures/font-probes-20261002"


class FontProbeTests(unittest.TestCase):
    def test_reproduce_uploaded_font_and_all_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            prepared = Path(directory) / "prepared"
            font_probe.prepare(prepared)
            for path in prepared.iterdir():
                self.assertEqual(
                    path.read_bytes(), (FIXTURES / path.name).read_bytes(), path.name
                )
            data = (prepared / "probe.ttf").read_bytes()
        self.assertEqual(font_probe.checksum(data), 0xB1B0AFBA)
        for i in range(struct.unpack_from(">H", data, 4)[0]):
            tag, expected, start, size = struct.unpack_from(">4sIII", data, 12 + 16 * i)
            table = bytearray(data[start : start + size])
            if tag == b"head":
                table[8:12] = bytes(4)
            self.assertEqual(font_probe.checksum(table), expected, tag)

    def test_programs_execute_with_independent_scales_without_clipping(self):
        manifest = json.loads((FIXTURES / "manifest.json").read_text())
        flags = analysis.calibration.MODES["native"] | freetype.FT_LOAD_PEDANTIC
        count = 0
        for page in manifest["pages"]:
            if page["group"] == "metrics":
                continue
            for p in page["probes"]:
                pixels, _ = analysis.calibration.render_glyph(
                    FIXTURES / "probe.ttf", p, flags
                )
                if p["name"] in ("instruction-witness", "ppem-x", "ppem-y"):
                    bounds = analysis.measure(pixels, p)["bounds"]
                    expected = {
                        "instruction-witness": 13,
                        "ppem-x": p["width"] or p["height"],
                        "ppem-y": p["height"],
                    }[p["name"]]
                    self.assertEqual(bounds[2] - bounds[0], expected, p)
                count += 1
        self.assertEqual(count, 373)
        # Independently check that the phantom-point instruction changes the
        # FreeType advance, so its lack of effect on printer spacing is meaningful.
        by_name = {g["name"]: g for g in manifest["glyphs"]}
        for height in (16, 32, 64):
            advances = []
            for name in ("advance-735", "advance-735-plus-one"):
                face = freetype.Face(str(FIXTURES / "probe.ttf"))
                face.set_pixel_sizes(0, height)
                face.load_char(chr(by_name[name]["codepoint"]), flags)
                advances.append(face.glyph.advance.x)
            self.assertEqual(advances[1] - advances[0], 64)

    def test_curve_subdivision_is_mathematically_identical(self):
        glyphs = {g["name"]: g for g in font_probe.glyphs()}

        def quadratic(a, b, c, t):
            return tuple(
                (1 - t) ** 2 * a[k] + 2 * (1 - t) * t * b[k] + t * t * c[k]
                for k in (0, 1)
            )

        for phase in (0, 32, 33):
            whole = glyphs[f"curve-whole-{phase}"]["contours"][0]
            split = glyphs[f"curve-split-{phase}"]["contours"][0]
            self.assertEqual(whole[:2], split[:2])
            self.assertEqual(whole[-1], split[-1])
            for i in range(65):
                t = Fraction(i, 64)
                a = quadratic(*whole[1:], t)
                b = (
                    quadratic(*split[1:4], t * 2)
                    if t <= Fraction(1, 2)
                    else quadratic(*split[3:], t * 2 - 1)
                )
                self.assertEqual(a, b)

    def test_replay_native_capture_evidence(self):
        report = analysis.analyze(FIXTURES, ["zd621", "zq610"])
        # The engine-specific baseline is intentionally pinned; newer engines
        # need a separate report rather than silently changing these observations.
        self.assertEqual(freetype.version(), (2, 13, 2))
        for device in report["devices"].values():
            self.assertEqual(
                device["totals"]["geometry"],
                dict(
                    underpaint=1179,
                    overpaint=647,
                    xor=1826,
                    union=15867,
                    exact_glyphs=175,
                    glyph_count=317,
                    foreground_iou=1 - 1826 / 15867,
                ),
            )
            self.assertEqual(device["totals"]["state"]["xor"], 48)
            self.assertEqual(
                [p["xor"] for p in device["bitmap_rotation"]], [297, 363, 220]
            )
            self.assertEqual(
                [p["xor"] for p in device["equivalent_curves"]], [1] + [0] * 14
            )
            for row in device["metrics"]:
                self.assertEqual(
                    row["measured_advance"], {16: 6, 32: 12, 64: 23}[row["height"]]
                )
        self.assertEqual(len(report["cross_device"]), 19)
        self.assertTrue(all(p["xor"] == 0 for p in report["cross_device"]))


if __name__ == "__main__":
    unittest.main()
