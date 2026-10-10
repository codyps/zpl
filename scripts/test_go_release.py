"""Go release safety regressions; all publication and proxy calls are mocked.

Tag and module contracts: https://go.dev/ref/mod#vcs-version
"""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("go_release", Path(__file__).with_name("go-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
remote_commit = release.remote_commit


class GoReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "zpl-go").mkdir()
        (self.root / "zpl-go/go.mod").write_text(f"module {release.MODULE}\n\ngo 1.25.0\n")
        self.version = self.enter(patch.object(release, "release_version", return_value="0.2.2"))
        self.output = self.enter(patch.object(release.subprocess, "check_output", side_effect=self.git_output))
        self.run = self.enter(patch.object(release.subprocess, "run"))
        self.remote = self.enter(patch.object(release, "remote_commit", side_effect=[None, "abc123"]))

    def enter(self, manager):
        result = manager.start()
        self.addCleanup(manager.stop)
        return result

    def git_output(self, args, **kwargs):
        if args[1] == "status":
            return ""
        if args[1] == "rev-parse":
            return "abc123\n"
        raise AssertionError(args)

    def test_ordinary_commit_skips_and_cannot_publish(self):
        self.version.return_value = None
        with contextlib.redirect_stdout(io.StringIO()) as out:
            release.prepare(self.root)
        self.assertEqual(out.getvalue(), "released=false\n")
        with self.assertRaisesRegex(ValueError, "No completed"):
            release.publish(self.root)
        self.run.assert_not_called()
        self.remote.assert_not_called()

    def test_new_tag_targets_release_and_downloads_from_public_proxy(self):
        release.publish(self.root)
        create, download = self.run.call_args_list
        self.assertEqual(create.args[0], ["gh", "api", "repos/codyps/zpl/git/refs", "--method", "POST",
                                         "-f", "ref=refs/tags/zpl-go/v0.2.2", "-f", "sha=abc123"])
        self.assertEqual(download.args[0], ["go", "mod", "download", release.MODULE + "@v0.2.2"])
        self.assertEqual(download.kwargs["env"]["GOPROXY"], "https://proxy.golang.org")
        self.assertEqual(download.kwargs["env"]["GONOPROXY"], "none")
        self.assertEqual(download.kwargs["env"]["GOWORK"], "off")
        self.assertNotEqual(download.kwargs["cwd"], self.root)

    def test_retry_existing_tag_still_registers_proxy(self):
        self.remote.side_effect = ["abc123", "abc123"]
        release.publish(self.root)
        self.assertEqual(self.run.call_count, 1)
        self.assertEqual(self.run.call_args.args[0][0], "go")

    def test_conflicting_tag_is_never_moved(self):
        self.remote.side_effect = ["other"]
        with self.assertRaisesRegex(ValueError, "Refusing to move"):
            release.publish(self.root)
        self.run.assert_not_called()

    def test_failed_creation_or_proxy_request_fails_visibly(self):
        for stage in ("gh", "go"):
            with self.subTest(stage=stage):
                self.remote.side_effect = [None, "abc123"]
                def run(args, **kwargs):
                    if args[0] == stage:
                        raise subprocess.CalledProcessError(1, args)
                self.run.side_effect = run
                with self.assertRaises(subprocess.CalledProcessError):
                    release.publish(self.root)

    def test_remote_verification_failure_prevents_proxy_request(self):
        self.remote.side_effect = [None, "other"]
        with self.assertRaisesRegex(ValueError, "does not match"):
            release.publish(self.root)
        self.assertEqual(self.run.call_count, 1)

    def test_dirty_checkout_is_not_published(self):
        self.output.side_effect = None
        self.output.return_value = " M zpl-go/guest.wasm\n"
        with self.assertRaisesRegex(ValueError, "clean checkout"):
            release.publish(self.root)
        self.remote.assert_not_called()

    def test_module_path_and_major_version_must_be_supported(self):
        self.version.return_value = "2.0.0"
        with self.assertRaisesRegex(ValueError, "migration"):
            release.publish(self.root)
        self.version.return_value = "0.2.2"
        (self.root / "zpl-go/go.mod").write_text("module example.org/other\n")
        with self.assertRaisesRegex(ValueError, "module path"):
            release.publish(self.root)
        self.run.assert_not_called()

    def test_remote_lightweight_and_annotated_tags(self):
        ref = "refs/tags/zpl-go/v0.2.2"
        self.output.side_effect = None
        for output, expected in [("", None), (f"abc123\t{ref}\n", "abc123"),
                                 (f"tagsha\t{ref}\nabc123\t{ref}^{{}}\n", "abc123")]:
            self.output.return_value = output
            self.assertEqual(remote_commit(self.root, "zpl-go/v0.2.2"), expected)


if __name__ == "__main__":
    unittest.main()
