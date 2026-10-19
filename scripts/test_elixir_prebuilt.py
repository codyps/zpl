"""Distribution completeness, checksum and immutable retry contracts."""

import importlib.util
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.request import urlopen

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
        # Model the action's single-library tarballs, without retaining a second
        # production implementation of its packaging or timestamp policy.
        self.dist.mkdir(exist_ok=True)
        paths = []
        for target in prebuilt.TARGETS:
            path = self.dist / prebuilt.filename("1.2.3", target)
            with tarfile.open(path, "w:gz") as archive:
                archive.add(self.library, arcname=path.name.removesuffix(".tar.gz"))
            paths.append(path)
        return paths

    def test_accept_upstream_archives_with_platform_specific_names(self):
        paths = self.build_all()
        self.assertEqual(set(prebuilt.artifacts("1.2.3", self.dist)), set(paths))
        self.assertTrue(
            prebuilt.filename("1.2.3", "x86_64-pc-windows-msvc").startswith(
                "zpl_elixir-"
            )
        )
        self.assertTrue(
            prebuilt.filename("1.2.3", "aarch64-apple-darwin").endswith(".so.tar.gz")
        )

    def test_pack_cross_compiled_library_uses_the_upstream_archive_contract(self):
        for target in prebuilt.TARGETS:
            prebuilt.pack('1.2.3', target, self.library, self.dist)
        for path in prebuilt.artifacts('1.2.3', self.dist):
            with tarfile.open(path) as archive:
                member = archive.getmembers()[0]
                self.assertEqual(archive.extractfile(member).read(), self.library.read_bytes())

    def test_every_target_is_required(self):
        paths = self.build_all()
        for path in paths:
            contents = path.read_bytes()
            path.unlink()
            with self.subTest(target=path.name), self.assertRaises(ValueError):
                prebuilt.artifacts('1.2.3', self.dist)
            path.write_bytes(contents)

    def test_reject_missing_extra_or_invalid_archives_before_generating_checksums(self):
        paths = self.build_all()
        checksum = "existing checksum manifest"
        (self.root / prebuilt.CHECKSUM).write_text(checksum)
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

    def test_upstream_checksum_task_downloads_the_exact_artifacts_from_loopback(self):
        paths = self.build_all()
        config_paths = []

        def run(args, **kwargs):
            self.assertEqual(kwargs["cwd"], self.root)
            if args[1] == "deps.get":
                return
            self.assertEqual(args[1:3], ["do", "loadconfig"])
            config = Path(args[3].removesuffix(","))
            config_paths.append(config)
            url = config.read_text().split('"')[1]
            self.assertTrue(url.startswith("http://127.0.0.1:"))
            self.assertEqual(
                args[4:],
                ["rustler_precompiled.download", "Zpl.Native", "--all", "--print"],
            )
            for path in paths:
                with urlopen(url + path.name) as response:
                    self.assertEqual(response.read(), path.read_bytes())

        with patch.object(prebuilt.subprocess, "run", side_effect=run):
            prebuilt.checksums("1.2.3", self.dist, self.root)
        self.assertEqual(len(config_paths), 1)
        self.assertFalse(config_paths[0].exists())

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
