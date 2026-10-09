"""PyPI release and retry contracts; no uploads, network, or account required."""

import contextlib
import hashlib
import importlib.util
import io
import json
import tempfile
import tomllib
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "python_release", Path(__file__).with_name("python-release.py")
)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / "zpl").mkdir()
        (self.root / "zpl-python").mkdir()
        (self.root / "zpl/Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
        self.manifest = self.root / "zpl-python/Cargo.toml"
        self.manifest.write_text('[package]\nname = "zpl-python"\nversion = "0.1.0"\n')
        (self.root / "zpl-python/pyproject.toml").write_text(
            '[project]\nname = "zplkit"\ndynamic = ["version"]\n'
        )
        self.lock = self.root / "Cargo.lock"
        self.lock.write_text(
            'version = 4\n\n[[package]]\nname = "zpl-python"\nversion = "0.1.0"\n'
            '\n[[package]]\nname = "zpl"\nversion = "1.2.3"\n'
        )

    def test_gate_skips_ordinary_commits_without_mutation(self):
        before = self.manifest.read_bytes(), self.lock.read_bytes()
        with (
            patch.object(release, "release_version", return_value=None),
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            release.prepare(self.root)
        self.assertEqual(output.getvalue(), "released=false\n")
        self.assertEqual(before, (self.manifest.read_bytes(), self.lock.read_bytes()))

    def test_completed_release_stamps_both_versions_and_is_idempotent(self):
        for _ in range(2):
            with (
                patch.object(release, "release_version", return_value="1.2.3"),
                contextlib.redirect_stdout(io.StringIO()) as output,
            ):
                release.prepare(self.root)
            self.assertEqual(output.getvalue(), "released=true\nversion=1.2.3\n")
            self.assertEqual(
                tomllib.loads(self.manifest.read_text())["package"]["version"], "1.2.3"
            )
            self.assertEqual(
                [p["version"] for p in tomllib.loads(self.lock.read_text())["package"]],
                ["1.2.3", "1.2.3"],
            )
        self.assertFalse((self.root / "zpl-python/Cargo.lock").exists())

    def test_wrong_release_versions_fail_before_mutation(self):
        for version in ["2.0.0", "1.2.3-rc.1", "01.2.3", "../1.2.3"]:
            before = self.manifest.read_bytes(), self.lock.read_bytes()
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.stamp(self.root, version)
            self.assertEqual(
                before, (self.manifest.read_bytes(), self.lock.read_bytes())
            )

    def test_bad_lock_fails_without_changing_manifest(self):
        self.lock.write_text("version = 4\n")
        before = self.manifest.read_bytes()
        with self.assertRaises(ValueError):
            release.stamp(self.root, "1.2.3")
        self.assertEqual(before, self.manifest.read_bytes())

    def test_wrong_project_name_fails(self):
        (self.root / "zpl-python/pyproject.toml").write_text(
            '[project]\nname = "another-package"\n'
        )
        with self.assertRaises(ValueError):
            release.stamp(self.root, "1.2.3")

    def artifacts(self):
        source = self.root / "dist"
        source.mkdir()
        platforms = [
            "manylinux_2_28_x86_64",
            "manylinux_2_28_aarch64",
            "macosx_10_12_x86_64",
            "macosx_11_0_arm64",
            "win_amd64",
        ]
        for platform in platforms:
            (source / f"zplkit-1.2.3-cp310-abi3-{platform}.whl").write_bytes(
                platform.encode()
            )
        (source / "zplkit-1.2.3.tar.gz").write_bytes(b"source")
        return source

    def remote(self, source):
        return {
            p.name: {
                "yanked": False,
                "digests": {"sha256": hashlib.sha256(p.read_bytes()).hexdigest()},
            }
            for p in source.iterdir()
        }

    def select(self, source, remote):
        destination = self.root / "upload"
        with (
            patch.object(release, "registry_files", return_value=remote),
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            release.select_uploads(source, destination, "1.2.3")
        return destination, output.getvalue()

    def test_new_release_uploads_complete_set(self):
        source = self.artifacts()
        destination, output = self.select(source, {})
        self.assertEqual(output, "publish=true\n")
        self.assertEqual(
            {p.name for p in source.iterdir()}, {p.name for p in destination.iterdir()}
        )

    def test_complete_release_skips_upload(self):
        source = self.artifacts()
        destination, output = self.select(source, self.remote(source))
        self.assertEqual(output, "publish=false\n")
        self.assertEqual(list(destination.iterdir()), [])

    def test_partial_release_uploads_only_missing_files(self):
        source = self.artifacts()
        remote = self.remote(source)
        missing = next(iter(remote))
        del remote[missing]
        destination, output = self.select(source, remote)
        self.assertEqual(output, "publish=true\n")
        self.assertEqual([p.name for p in destination.iterdir()], [missing])

    def test_hash_mismatch_or_yanked_file_stops_release(self):
        source = self.artifacts()
        for field, value in [("yanked", True), ("digests", {"sha256": "wrong"})]:
            remote = self.remote(source)
            next(iter(remote.values()))[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.select(source, remote)
            self.assertFalse((self.root / "upload").exists())

    def test_missing_build_artifact_stops_release(self):
        source = self.artifacts()
        next(source.iterdir()).unlink()
        with self.assertRaises(ValueError):
            self.select(source, {})

    def test_only_registry_404_means_missing(self):
        for status in [404, 403, 429, 500]:
            error = urllib.error.HTTPError(
                "https://pypi.org", status, "error", None, None
            )
            with (
                self.subTest(status=status),
                patch.object(release.urllib.request, "urlopen", side_effect=error),
            ):
                if status == 404:
                    self.assertEqual(release.registry_files("1.2.3"), {})
                else:
                    with self.assertRaises(urllib.error.HTTPError):
                        release.registry_files("1.2.3")

    def test_registry_network_and_invalid_json_errors_propagate(self):
        with patch.object(release.urllib.request, "urlopen", side_effect=TimeoutError):
            with self.assertRaises(TimeoutError):
                release.registry_files("1.2.3")
        with patch.object(
            release.urllib.request, "urlopen", return_value=io.BytesIO(b"not json")
        ):
            with self.assertRaises(json.JSONDecodeError):
                release.registry_files("1.2.3")

    def test_registry_version_mismatch_stops_release(self):
        data = {"info": {"name": "zplkit", "version": "9.9.9"}, "urls": []}
        with patch.object(
            release.urllib.request,
            "urlopen",
            return_value=io.BytesIO(json.dumps(data).encode()),
        ):
            with self.assertRaises(ValueError):
                release.registry_files("1.2.3")


if __name__ == "__main__":
    unittest.main()
