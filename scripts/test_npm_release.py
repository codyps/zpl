"""Release-gate regression tests; no registry access or publication."""
import contextlib
import importlib.util
import io
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("npm_release", Path(__file__).with_name("npm-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "zpl").mkdir()
        (self.root / "zpl-node").mkdir()
        (self.root / "zpl/Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
        self.package = self.root / "zpl-node/package.json"
        self.package.write_text('{"name":"@codyps/zpl","version":"0.1.0"}')
        self.git("init", "--quiet")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "test: create release fixture")
        original = subprocess.check_output
        self.github = {"tagName": "zpl-v1.2.3", "isDraft": False, "isPrerelease": False}

        def output(args, **kwargs):
            if args[0] == "gh":
                return json.dumps(self.github)
            return original(args, **kwargs)

        mock = patch("renderer_release.subprocess.check_output", side_effect=output)
        self.calls = mock.start()
        self.addCleanup(mock.stop)

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def test_ordinary_commit_does_not_publish_or_change_manifest(self):
        before = self.package.read_bytes()
        with contextlib.redirect_stdout(io.StringIO()) as output:
            release.prepare(self.root)
        self.assertEqual(output.getvalue(), "released=false\n")
        self.assertEqual(self.package.read_bytes(), before)
        self.calls.assert_not_called()

    def test_previous_release_tag_does_not_publish_new_commit(self):
        self.git("tag", "zpl-v1.2.3")
        self.git("commit", "--quiet", "--allow-empty", "-m", "fix: later change")
        self.assertIsNone(release.release_version(self.root))

    def test_completed_release_sets_version_and_can_be_retried(self):
        self.git("tag", "-a", "zpl-v1.2.3", "-m", "release")
        for _ in range(2):
            with contextlib.redirect_stdout(io.StringIO()) as output:
                release.prepare(self.root)
            self.assertEqual(output.getvalue(), "released=true\nversion=1.2.3\n")
            self.assertEqual(json.loads(self.package.read_text())["version"], "1.2.3")

    def test_lightweight_release_tag(self):
        self.git("tag", "zpl-v1.2.3")
        self.assertEqual(release.release_version(self.root), "1.2.3")

    def test_incomplete_or_mismatched_github_release_fails(self):
        self.git("tag", "zpl-v1.2.3")
        for key, value in [("isDraft", True), ("isPrerelease", True), ("tagName", "zpl-v9.9.9")]:
            with self.subTest(key=key), patch.dict(self.github, {key: value}):
                with self.assertRaises(ValueError):
                    release.release_version(self.root)

    def test_missing_github_release_fails_instead_of_skipping(self):
        self.git("tag", "zpl-v1.2.3")
        original = self.calls.side_effect

        def failed(args, **kwargs):
            if args[0] == "gh":
                raise subprocess.CalledProcessError(1, args)
            return original(args, **kwargs)

        self.calls.side_effect = failed
        with self.assertRaises(subprocess.CalledProcessError):
            release.release_version(self.root)

    def test_prerelease_is_not_published_as_latest(self):
        (self.root / "zpl/Cargo.toml").write_text('[package]\nversion = "1.2.3-rc.1"\n')
        self.assertIsNone(release.release_version(self.root))
        self.calls.assert_not_called()


if __name__ == "__main__":
    unittest.main()
