import importlib.util
from pathlib import Path
import struct
import unittest
import zlib
import tempfile
import threading
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer

spec = importlib.util.spec_from_file_location("capture", Path(__file__).with_name("capture-printer-previews.py"))
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


class CaptureTests(unittest.TestCase):
    def test_width_and_payload(self):
        source = b"^XA^PW832^FO800,20^FDliteral ^PW832~value^FS^PW120^XZ"
        result, widths = capture.adapt(source, 384, 300)
        self.assertEqual(widths, [832, 120])
        self.assertIn(b"^PW384^FO800,20", result)
        self.assertIn(b"^FDliteral ^PW832~value^FS^PW120", result)
        self.assertIn(b"^BY2,3,10", result)

    def test_binary_payload_not_rewritten(self):
        source = b"^XA^GFB,8,8,1,^PW832~A^FS^PW832^XZ"
        result, widths = capture.adapt(source, 384, 200)
        self.assertIn(b"^GFB,8,8,1,^PW832~A^FS^PW384", result)
        self.assertEqual(widths, [832])

    def test_unsafe_and_unsupported(self):
        for source in (b"~DYR:X^XA^XZ", b"^XA^JUS^XZ", b"^XA^FDx\0y^FS^XZ", b"^XA^CC!^XZ", b"^XA^XZ^XA^XZ", b"^XA^GFB,999,999,1,abc^XZ"):
            with self.subTest(source=source), self.assertRaises(ValueError):
                capture.adapt(source, 384, 200)

    def test_png_crc(self):
        def chunk(kind, payload):
            return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))
        png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 384, 200, 8, 0, 0, 0, 0)) + chunk(b"IEND", b"")
        self.assertEqual(capture.png_dimensions(png), [384, 200])
        with self.assertRaises(ValueError):
            capture.png_dimensions(png[:-1] + b"x")

    def test_origin(self):
        for host in ("file:///tmp/a", "http://user:pass@printer/", "http://printer/path"):
            with self.assertRaises(ValueError):
                capture.Printer(host)

    def test_http_preview_form_and_image(self):
        requests = []
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                form = urllib.parse.parse_qs(self.rfile.read(int(self.headers["Content-Length"])))
                requests.append((self.path, form))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'<IMG SRC="png?prev=Y&amp;dev=R">')

            def do_GET(self):
                requests.append(self.path)
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"test-image")

        server = HTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            printer = capture.Printer(f"http://127.0.0.1:{server.server_port}/")
            _, _, data = printer.preview(b"^XA^FDabc^FS^XZ")
            self.assertEqual(data, b"test-image")
            self.assertEqual(requests[0][0], "/zpl")
            self.assertEqual(requests[0][1][b"prev"], [b"Preview Label"])
            self.assertNotIn(b"print", requests[0][1])
            self.assertEqual(requests[1], "/png?prev=Y&dev=R")
            with self.assertRaises(ValueError):
                printer.fetch("http://other.invalid/image")
        finally:
            server.shutdown()
            thread.join()
            server.server_close()

    def test_saved_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "plan.json").write_bytes(b"{}")
            (root / "image.png").write_bytes(b"original")
            capture.save(root / "provenance.json", {
                "plan_sha256": capture.sha(b"{}"),
                "files": {"image.png": capture.sha(b"original")},
            })
            capture.verify(root)
            (root / "image.png").write_bytes(b"modified")
            with self.assertRaisesRegex(ValueError, "hash mismatch"):
                capture.verify(root)


if __name__ == "__main__":
    unittest.main()
