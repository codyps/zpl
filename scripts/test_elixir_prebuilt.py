"""Distribution completeness, checksum and immutable retry contracts."""

import hashlib
import importlib.util
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "prebuilt", Path(__file__).with_name("elixir-prebuilt.py")
)
prebuilt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prebuilt)


class PrebuiltTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.library = self.root / "native"
        self.library.write_bytes(b"native library fixture")
        self.dist = self.root / "dist"

    def build_all(self):
        return [
            prebuilt.pack("1.2.3", target, self.library, self.dist)
            for target in prebuilt.TARGETS
        ]

    def test_archives_are_deterministic_and_use_loader_filenames(self):
        for path in self.build_all():
            original = path.read_bytes()
            target = next(t for t in prebuilt.TARGETS if t in path.name)
            prebuilt.pack("1.2.3", target, self.library, self.dist)
            self.assertEqual(path.read_bytes(), original)
            with tarfile.open(path) as archive:
                self.assertEqual(
                    archive.getnames(), [path.name.removesuffix(".tar.gz")]
                )
                self.assertEqual(
                    archive.extractfile(archive.getmembers()[0]).read(),
                    self.library.read_bytes(),
                )
        self.assertTrue(
            prebuilt.filename("1.2.3", "x86_64-pc-windows-msvc").startswith(
                "zpl_elixir-"
            )
        )
        self.assertTrue(
            prebuilt.filename("1.2.3", "aarch64-apple-darwin").endswith(".so.tar.gz")
        )

    def test_checksums_pin_every_archive_and_reject_missing_extra_or_invalid(self):
        paths = self.build_all()
        prebuilt.checksums("1.2.3", self.dist, self.root)
        checksum = (self.root / prebuilt.CHECKSUM).read_text()
        for path in paths:
            self.assertIn(
                f'"{path.name}" => "sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"',
                checksum,
            )
        paths[0].unlink()
        with self.assertRaises(ValueError):
            prebuilt.checksums("1.2.3", self.dist, self.root)
        self.build_all()
        (self.dist / "unexpected.tar.gz").write_bytes(b"unexpected")
        with self.assertRaises(ValueError):
            prebuilt.checksums("1.2.3", self.dist, self.root)
        (self.dist / "unexpected.tar.gz").unlink()
        with tarfile.open(paths[0], "w:gz") as archive:
            archive.add(self.library, arcname="../wrong.so")
        with self.assertRaises(ValueError):
            prebuilt.checksums("1.2.3", self.dist, self.root)
        self.assertEqual((self.root / prebuilt.CHECKSUM).read_text(), checksum)

    def test_upload_only_missing_and_never_overwrite_changed_assets(self):
        paths = self.build_all()
        existing = paths[0]
        metadata = {
            "tagName": "zpl-v1.2.3",
            "isDraft": False,
            "isPrerelease": False,
            "assets": [{"name": existing.name}],
        }
        for changed in [False, True]:

            def run(args, changed=changed, **kwargs):
                if args[2] == "download":
                    (Path(args[-1]) / existing.name).write_bytes(
                        b"different" if changed else existing.read_bytes()
                    )

            with (
                patch.object(
                    prebuilt.subprocess,
                    "check_output",
                    return_value=json.dumps(metadata),
                ),
                patch.object(prebuilt.subprocess, "run", side_effect=run) as commands,
            ):
                if changed:
                    with self.assertRaises(ValueError):
                        prebuilt.publish("1.2.3", self.dist)
                    self.assertFalse(
                        any(
                            call.args[0][2] == "upload"
                            for call in commands.call_args_list
                        )
                    )
                else:
                    prebuilt.publish("1.2.3", self.dist)
                    upload = commands.call_args.args[0]
                    self.assertEqual(upload[2], "upload")
                    self.assertNotIn(str(existing), upload)
                    self.assertNotIn("--clobber", upload)
                    for path in paths[1:]:
                        self.assertIn(str(path), upload)


if __name__ == "__main__":
    unittest.main()
