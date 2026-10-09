"""Binding contracts; renderer geometry stays in the Rust regression corpus.

Framing and raster contracts: docs/parser-coverage.md and docs/local-renderer.md.
"""

import gc
import math
import struct
import unittest
from concurrent.futures import ThreadPoolExecutor
from importlib.resources import files

from zplkit.output import Limits as OutputLimits
from zplkit.output import OutputError
from zplkit.parse import ParseError, Syntax, parse
from zplkit.rendering import Compatibility, Limits, RenderError

import zplkit
from zplkit import Options, render


class BindingsTest(unittest.TestCase):
    def setUp(self):
        self.options = Options(profile="specification", width=16, height=12)
        self.source = b"^XA^FO2,3^GB4,2,2^FS^XZ"

    def test_outputs_and_pixel_geometry(self):
        document = render(self.source, self.options)
        self.assertEqual(document.warnings, [])
        (scene,) = document.labels
        self.assertEqual((scene.width, scene.height, scene.dpi), (16, 12, 203))
        png = scene.png()
        self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(struct.unpack(">II", png[16:24]), (16, 12))
        self.assertIn(b"<svg", scene.svg())
        self.assertEqual(scene.pdf(), document.pdf())
        self.assertTrue(scene.pdf().startswith(b"%PDF-"))
        raster = scene.rasterize()
        expected = bytes(
            0 if 2 <= x < 6 and 3 <= y < 5 else 255
            for y in range(12)
            for x in range(16)
        )
        self.assertEqual(raster.pixels, expected)
        self.assertEqual((raster.width, raster.height), (16, 12))

    def test_multipage_and_scene_lifetime(self):
        document = render(self.source + b"^XA^PW8^LL9^XZ", self.options)
        self.assertEqual(len(document.labels), 2)
        self.assertEqual((document.labels[1].width, document.labels[1].height), (8, 9))
        self.assertIn(b"/Count 2", document.pdf())
        scene = document.labels[0]
        before = scene.png()
        del document
        gc.collect()
        self.assertEqual(scene.png(), before)

    def test_profiles_and_immutable_overrides(self):
        self.assertFalse(Compatibility().qr_printer_mask_selection)
        default = Options()
        self.assertEqual((default.width, default.height, default.dpi), (832, 1218, 203))
        self.assertTrue(default.compatibility.qr_printer_mask_selection)
        self.assertFalse(self.options.compatibility.qr_printer_mask_selection)
        mobile = Options(profile="zq610-plus")
        self.assertEqual((mobile.width, mobile.height), (384, 2030))
        custom = default.compatibility.replace(
            qr_printer_mask_selection=False, macro_pdf417_file_id=(1, 2, 3)
        )
        self.assertFalse(
            Options(compatibility=custom).compatibility.qr_printer_mask_selection
        )
        self.assertEqual(tuple(custom.macro_pdf417_file_id), (1, 2, 3))
        self.assertTrue(default.compatibility.qr_printer_mask_selection)
        with self.assertRaises(AttributeError):
            default.width = 1
        # Independently selectable measured behavior: zero-thickness boxes.
        source = b"^XA^FO2,3^GB4,2,0^FS^XZ"
        with self.assertRaisesRegex(RenderError, "invalid shape dimensions"):
            render(source, self.options)
        compat = Options(
            profile="specification",
            width=16,
            height=12,
            compatibility=self.options.compatibility.replace(
                box_zero_thickness_as_one=True
            ),
        )
        measured = render(source, compat).labels[0].rasterize().pixels
        self.assertIn(0, measured)

    def test_font_warnings_are_preserved(self):
        document = render(b"^XA^FDHello^FS^XZ", self.options)
        self.assertTrue(any("captured bitmap strikes" in w for w in document.warnings))

    def test_str_is_utf8_and_bytes_are_unchanged(self):
        source = "^XA^CI28^FDé^FS^XZ"
        self.assertEqual(
            render(source, self.options).labels[0].png(),
            render(source.encode(), self.options).labels[0].png(),
        )
        parsed = parse(source)
        self.assertEqual(b"".join(e.data for e in parsed.elements), source.encode())

    def test_lossless_binary_unknown_commands_and_offsets(self):
        source = b" \n^XA^ZZunknown^GFB,4,4,4,^~\0\xff^FS\x03"
        result = parse(source)
        self.assertEqual(b"".join(bytes(e) for e in result.elements), source)
        self.assertEqual(result.elements[0].kind, "before_first_command")
        self.assertEqual(result.elements[-1].kind, "control_character")
        offset = 0
        for element in result.elements:
            self.assertEqual(element.offset, offset)
            offset += len(element.data)
        self.assertIn(b"^GFB,4,4,4,^~\0\xff", [e.data for e in result.elements])

    def test_syntax_changes_and_continuation(self):
        result = parse(b"^CC!!XA!CD;!FO1;2!XZ")
        self.assertEqual(result.syntax.format_prefix, ord("!"))
        self.assertEqual(result.syntax.delimiter, ord(";"))
        continued = parse(b"!XA!XZ", syntax=result.syntax)
        self.assertEqual([e.data for e in continued.elements], [b"!XA", b"!XZ"])
        initial = Syntax(format_prefix=ord("!"))
        self.assertEqual(
            parse(b"!XA", syntax=initial).elements[0].kind, "format_command"
        )

    def test_errors_preserve_diagnostics(self):
        with self.assertRaises(ParseError) as caught:
            parse(b"^XA^GFB,4,4,4,ab")
        self.assertEqual(caught.exception.offset, 3)
        self.assertEqual(caught.exception.kind, "TruncatedBinaryData")
        with self.assertRaises(RenderError) as caught:
            render(b"^XA^ZZ^XZ", self.options)
        self.assertEqual(caught.exception.offset, 3)
        self.assertIn(caught.exception.message, str(caught.exception))
        self.assertIsInstance(caught.exception, ValueError)

    def test_render_and_output_budgets(self):
        for limits in [
            Limits(input_bytes=1),
            Limits(labels=0),
            Limits(pixels=1),
            Limits(segments=0),
            Limits(dimension=1),
        ]:
            with self.subTest(limits=repr(limits)), self.assertRaises(RenderError):
                render(self.source, self.options, limits=limits)
        document = render(self.source, self.options)
        scene = document.labels[0]
        for method in [scene.png, scene.svg, scene.pdf, scene.rasterize]:
            with self.subTest(method=method), self.assertRaises(OutputError):
                method(limits=OutputLimits(pixels=1))
        with self.assertRaises(OutputError):
            document.pdf(limits=OutputLimits(pages=0))
        with self.assertRaises(RenderError):
            render(b"^XA^PW100^XZ", self.options, limits=Limits(dimension=32))
        self.assertEqual(Limits().replace(labels=2).labels, 2)
        self.assertEqual(Limits().labels, 64)

    def test_invalid_arguments(self):
        for source in [None, 42, bytearray(b"^XA^XZ")]:
            with self.subTest(source=source), self.assertRaises(TypeError):
                render(source)
        for kwargs in [{"width": 0}, {"dpi": 0}, {"profile": "missing"}]:
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                Options(**kwargs)
        with self.assertRaises(OverflowError):
            Options(width=-1)
        with self.assertRaises(OverflowError):
            Syntax(delimiter=256)
        with self.assertRaises(TypeError):
            Limits(unknown_field=1)
        for number in [math.nan, math.inf, -1.0]:
            with self.subTest(number=number), self.assertRaises(ValueError):
                render(self.source, self.options, limits=Limits(number_abs=number))
            with self.assertRaises(ValueError):
                render(self.source, self.options).pdf(
                    limits=OutputLimits(coordinate_abs=number)
                )

    def test_parallel_shared_scene(self):
        scene = render(self.source, self.options).labels[0]
        with ThreadPoolExecutor(max_workers=4) as pool:
            images = list(pool.map(lambda _: scene.png(), range(12)))
        self.assertTrue(all(image == images[0] for image in images))

    def test_empty_input_and_package_metadata(self):
        self.assertEqual(parse(b"").elements, [])
        with self.assertRaisesRegex(RenderError, "no labels"):
            render(b"", self.options)
        self.assertTrue(zplkit.__version__)
        self.assertTrue(zplkit.library_version)
        self.assertTrue(files("zplkit").joinpath("py.typed").is_file())
        self.assertTrue(files("zplkit").joinpath("_native.pyi").is_file())


if __name__ == "__main__":
    unittest.main()
