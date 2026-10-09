"""Hex version, artifact and retry contracts; mocked HTTP, no live uploads."""

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import tarfile
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "elixir_release", Path(__file__).with_name("elixir-release.py")
)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "zpl").mkdir()
        (self.root / "zpl-elixir").mkdir()
        (self.root / "zpl/Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
        self.manifest = self.root / "zpl-elixir/Cargo.toml"
        self.manifest.write_text('[package]\nname = "zpl_elixir"\nversion = "0.1.0"\n')
        self.mix = self.root / "zpl-elixir/mix.exs"
        self.mix.write_text('app: :zpl,\nversion: "0.1.0",\n')
        self.lock = self.root / "Cargo.lock"
        self.lock.write_text(
            'version = 4\n[[package]]\nname = "zpl_elixir"\nversion = "0.1.0"\n'
        )

    def test_stamp_all_versions_idempotently(self):
        release.stamp(self.root, "1.2.3")
        release.stamp(self.root, "1.2.3")
        for path in [self.manifest, self.mix, self.lock]:
            self.assertIn('"1.2.3"', path.read_text())
        self.assertFalse((self.root / "zpl-elixir/Cargo.lock").exists())

    def test_invalid_version_fails_before_writes(self):
        for version in ["2.0.0", "1.2.3-rc.1", "01.2.3", "../1.2.3"]:
            before = [p.read_bytes() for p in [self.manifest, self.mix, self.lock]]
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.stamp(self.root, version)
            self.assertEqual(
                before, [p.read_bytes() for p in [self.manifest, self.mix, self.lock]]
            )

    def test_inconsistent_metadata_fails_before_writes(self):
        for path in [self.manifest, self.mix, self.lock]:
            original = path.read_bytes()
            path.write_text("invalid metadata")
            before = [p.read_bytes() for p in [self.manifest, self.mix, self.lock]]
            with self.subTest(path=path), self.assertRaises((ValueError, KeyError)):
                release.stamp(self.root, "1.2.3")
            self.assertEqual(
                before, [p.read_bytes() for p in [self.manifest, self.mix, self.lock]]
            )
            path.write_bytes(original)

    def artifact(self, name="zpl", version="1.2.3"):
        path = self.root / "zpl-1.2.3.tar"
        metadata = f'{{<<"name">>,<<"{name}">>}}.\n{{<<"app">>,<<"zpl">>}}.\n{{<<"version">>,<<"{version}">>}}.\n'.encode()
        with tarfile.open(path, "w") as archive:
            entry = tarfile.TarInfo("metadata.config")
            entry.size = len(metadata)
            archive.addfile(entry, io.BytesIO(metadata))
        return path

    def metadata(self, artifact):
        return {
            "version": "1.2.3",
            "checksum": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            "retirement": None,
        }

    def test_existing_identical_version_skips_post_without_credentials(self):
        artifact = self.artifact()
        with (
            patch.object(
                release, "registry_release", return_value=self.metadata(artifact)
            ),
            patch.object(release.urllib.request, "urlopen") as post,
        ):
            release.publish(artifact, "1.2.3")
        post.assert_not_called()

    def test_existing_changed_retired_or_wrong_version_fails(self):
        artifact = self.artifact()
        for key, value in [
            ("checksum", "wrong"),
            ("retirement", {}),
            ("version", "2.0.0"),
        ]:
            metadata = self.metadata(artifact) | {key: value}
            with (
                self.subTest(key=key),
                patch.object(release, "registry_release", return_value=metadata),
                self.assertRaises(ValueError),
            ):
                release.publish(artifact, "1.2.3")

    def test_publishes_exact_artifact_once_without_replacement(self):
        artifact = self.artifact()
        with (
            patch.object(release, "registry_release", return_value=None),
            patch.dict(os.environ, {"HEX_API_KEY": "test-key"}),
            patch.object(
                release.urllib.request,
                "urlopen",
                return_value=io.BytesIO(json.dumps(self.metadata(artifact)).encode()),
            ) as post,
        ):
            release.publish(artifact, "1.2.3")
        post.assert_called_once()
        request = post.call_args.args[0]
        self.assertEqual(request.data, artifact.read_bytes())
        self.assertEqual(
            request.full_url, "https://hex.pm/api/packages/zpl/releases?replace=false"
        )
        self.assertEqual(request.get_header("Authorization"), "test-key")

    def test_missing_key_fails_without_post(self):
        with (
            patch.object(release, "registry_release", return_value=None),
            patch.dict(os.environ, {}, clear=True),
            patch.object(release.urllib.request, "urlopen") as post,
            self.assertRaisesRegex(ValueError, "HEX_API_KEY"),
        ):
            release.publish(self.artifact(), "1.2.3")
        post.assert_not_called()

    def test_check_only_never_posts(self):
        with (
            patch.object(release, "registry_release", return_value=None),
            patch.object(release.urllib.request, "urlopen") as post,
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            release.publish(self.artifact(), "1.2.3", check_only=True)
        self.assertEqual(output.getvalue(), "publish=true\n")
        post.assert_not_called()

    def test_wrong_archive_name_or_version_fails_before_network(self):
        for name, version in [("different", "1.2.3"), ("zpl", "1.2.4")]:
            with (
                self.subTest(name=name),
                patch.object(release, "registry_release") as get,
                self.assertRaises(ValueError),
            ):
                release.publish(self.artifact(name, version), "1.2.3")
            get.assert_not_called()

    def test_only_404_means_missing(self):
        for code in [404, 403, 429, 500]:
            error = urllib.error.HTTPError("https://hex.pm", code, "error", None, None)
            with (
                self.subTest(code=code),
                patch.object(release.urllib.request, "urlopen", side_effect=error),
            ):
                if code == 404:
                    self.assertIsNone(release.registry_release("1.2.3"))
                else:
                    with self.assertRaises(urllib.error.HTTPError):
                        release.registry_release("1.2.3")

    def test_network_and_malformed_metadata_fail(self):
        for error in [TimeoutError(), json.JSONDecodeError("bad", "", 0)]:
            with (
                patch.object(release.urllib.request, "urlopen", side_effect=error),
                self.assertRaises(type(error)),
            ):
                release.registry_release("1.2.3")

    def test_failed_upload_is_not_retried(self):
        with (
            patch.object(release, "registry_release", return_value=None),
            patch.dict(os.environ, {"HEX_API_KEY": "test-key"}),
            patch.object(
                release.urllib.request, "urlopen", side_effect=TimeoutError
            ) as post,
            self.assertRaises(TimeoutError),
        ):
            release.publish(self.artifact(), "1.2.3")
        post.assert_called_once()

    def test_ordinary_commit_does_not_stamp(self):
        with (
            patch.object(release, "release_version", return_value=None),
            patch.object(release, "stamp") as stamp,
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            release.prepare(self.root)
        self.assertEqual(output.getvalue(), "released=false\n")
        stamp.assert_not_called()


if __name__ == "__main__":
    unittest.main()
