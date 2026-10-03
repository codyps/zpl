"""Offline checks for reproducible device probes and outline initialization."""

import json
from pathlib import Path
import tempfile
import unittest

import cvt_axis_probe
import small_font_probe
import analyze_font0_outline
import font0_outline_probe
import rounding_probe
import scale_probe
import swiss_probe

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures"


class SwissProbeTests(unittest.TestCase):
    def test_reproduce_fonts_and_requests(self):
        cases = [
            ("rounding-font-20261003", rounding_probe.prepare),
            ("scaling-font-20261003", scale_probe.prepare),
            ("scaling-font-validation-20261003", lambda p: scale_probe.prepare(p, 65)),
            ("swiss-font-20261003", swiss_probe.prepare),
            ("font0-outline-20261003", font0_outline_probe.prepare),
        ]
        cases += [
            ("cvt-axis-20261003", cvt_axis_probe.prepare),
            ("cvt-axis-validation-20261003", lambda p: cvt_axis_probe.prepare(p, True)),
            (
                "cvt-axis-validation-v2-20261003",
                lambda p: cvt_axis_probe.prepare(p, True, "R:ZP26E.TTF"),
            ),
            ("small-font-20261003", small_font_probe.prepare),
            (
                "small-font-validation-20261003",
                lambda p: small_font_probe.prepare(p, True),
            ),
        ]
        for name, prepare in cases:
            with self.subTest(
                campaign=name
            ), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "prepared"
                prepare(output)
                for file in output.iterdir():
                    self.assertEqual(
                        file.read_bytes(),
                        (ROOT / name / file.name).read_bytes(),
                        file.name,
                    )

    def test_trace_holes_and_diagonal_contacts(self):
        ring = {
            (x, y) for x in range(5) for y in range(5) if x in (0, 4) or y in (0, 4)
        }
        loops = analyze_font0_outline.trace(ring | {(5, 5)})
        self.assertEqual(len(loops), 3)
        area = (
            lambda loop: sum(
                a[0] * b[1] - a[1] * b[0] for a, b in zip(loop, loop[1:] + loop[:1])
            )
            // 2
        )
        self.assertEqual(sorted(map(area, loops)), [-9, 1, 25])
        self.assertEqual(sum(map(area, loops)), len(ring) + 1)
        self.assertEqual(
            len(
                analyze_font0_outline.contours(
                    {(x, y) for x in range(20) for y in range(10)}
                )[0]
            ),
            4,
        )

    def test_reserved_sizes_are_disjoint(self):
        for name in ["swiss-font-20261003", "font0-outline-20261003"]:
            manifest = json.loads((ROOT / name / "manifest.json").read_text())
            groups = {
                group: {
                    (p["width"], p["height"])
                    for page in manifest["pages"]
                    if page["group"] == group
                    for p in page["probes"]
                }
                for group in ["development", "validation"]
            }
            self.assertTrue(groups["validation"])
            self.assertFalse(groups["development"] & groups["validation"])


if __name__ == "__main__":
    unittest.main()
