"""Binding-only release regressions using real git history; no remote writes."""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest

spec = importlib.util.spec_from_file_location("bindings_release", Path(__file__).with_name("bindings-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class BindingReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.git("init", "-b", "main", "--quiet")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Release Test")
        members = [*release.CRATES, "consumer", "raster-diff", "zpl-bitmap-fonts"]
        self.write("Cargo.toml", '[workspace]\nresolver = "2"\nmembers = ' + json.dumps(members) + '\n')
        self.write("release-plz.toml", '[workspace]\nrelease = false\ndependencies_update = false\nrepo_url = "https://github.com/codyps/zpl"\n\n[[package]]\nname = "zpl"\nrelease = true\nsemver_check = false\n')
        lock = 'version = 4\n'
        for member in members:
            name = release.CRATES.get(member, member)
            text = f'[package]\nname = "{name}"\nversion = "0.2.1"\nedition = "2021"\n'
            if member != "zpl":
                text += 'publish = false\n\n[dependencies]\nzpl = { version = "0.2.1", path = "../zpl" }\n'
            self.write(f"{member}/Cargo.toml", text)
            self.write(f"{member}/src/lib.rs", 'pub fn render() {}\n')
            self.write(f"{member}/CHANGELOG.md", '# Changelog\n\n## [Unreleased]\n\n## [0.2.1] - 2026-10-09\n\n- Previous release.\n')
            lock += f'\n[[package]]\nname = "{name}"\nversion = "0.2.1"\n'
            if member != "zpl":
                lock += 'dependencies = ["zpl"]\n'
        self.write("Cargo.lock", lock)
        self.write("zpl-node/package.json", '{"name": "@codyps/zpl", "version": "0.2.1"}\n')
        self.write("zpl-node/index.cjs", '// Initial wrapper\n')
        self.write("zpl-elixir/mix.exs", 'app: :zpl,\nversion: "0.2.1",\n')
        self.commit("chore: prepare release")
        self.git("tag", "zpl-v0.2.1")

    def git(self, *args):
        return release.git(self.root, *args)

    def write(self, file, text):
        path = self.root / file
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self, message):
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", message)

    def snapshot(self):
        return {str(p.relative_to(self.root)): p.read_bytes() for p in self.root.rglob("*")
                if p.is_file() and ".git" not in p.parts}

    def assert_versions(self, expected):
        for directory, name in release.CRATES.items():
            manifest = tomllib.loads((self.root / directory / "Cargo.toml").read_text())
            self.assertEqual(manifest["package"]["version"], expected, name)
        lock = tomllib.loads((self.root / "Cargo.lock").read_text())
        for package in lock["package"]:
            if package["name"] in release.CRATES.values():
                self.assertEqual(package["version"], expected)
        self.assertEqual(json.loads((self.root / "zpl-node/package.json").read_text())["version"], expected)
        self.assertIn(f'version: "{expected}"', (self.root / "zpl-elixir/mix.exs").read_text())
        consumer = tomllib.loads((self.root / "consumer/Cargo.toml").read_text())
        self.assertEqual(consumer["dependencies"]["zpl"]["version"], expected)
        self.assertEqual(consumer["package"]["version"], "0.2.1")

    def test_each_binding_and_packaging_path_triggers_patch(self):
        for binding in release.PATHS:
            with self.subTest(binding=binding):
                self.git("reset", "--hard", "zpl-v0.2.1")
                self.git("clean", "-fd")
                path = binding if binding.endswith(".py") or binding.endswith(".sh") else f"{binding}/new-file"
                self.write(path, 'new binding functionality\n')
                self.commit("feat(binding): add capability")
                release.prepare(self.root)
                self.assert_versions("0.2.2")
                notes = (self.root / "zpl/CHANGELOG.md").read_text()
                self.assertIn('feat(binding): add capability', notes)
                self.assertIn('zpl-v0.2.1...zpl-v0.2.2', notes)
                self.assertIn('## [0.2.1] - 2026-10-09\n\n- Previous release.', notes)

    def test_breaking_subject_and_footer_require_minor_before_one(self):
        config = tomllib.loads((self.root / 'release-plz.toml').read_text())
        for message in ('feat(node)!: remove old API', 'fix(c): change layout\n\nBREAKING CHANGE: layout changed',
                        'feat(elixir): replace API\n\nBREAKING-CHANGE: remove old API'):
            self.assertEqual(release.next_binding_version((0, 2, 1), [('abc', message)], config), (0, 3, 0))
        self.assertEqual(release.next_binding_version((0, 0, 1), [('abc', 'fix!: break')], config), (0, 0, 2))
        self.assertEqual(release.next_binding_version((1, 2, 1), [('abc', 'fix!: break')], config), (2, 0, 0))
        self.assertEqual(release.next_binding_version((1, 2, 1), [('abc', 'feat: add')], config), (1, 3, 0))
        self.assertEqual(release.next_binding_version((1, 2, 1), [('abc', 'fix: repair')], config), (1, 2, 2))

    def test_feature_policy_override_and_unsupported_policy_fail(self):
        config = {'workspace': {'features_always_increment_minor': True}, 'package': [{'name': 'zpl'}]}
        self.assertEqual(release.next_binding_version((0, 2, 1), [('abc', 'feat: add')], config), (0, 3, 0))
        config['workspace']['custom_major_increment_regex'] = '^major'
        with self.assertRaisesRegex(ValueError, 'policy'):
            release.next_binding_version((0, 2, 1), [('abc', 'major: add')], config)

    def test_no_changes_or_unrelated_changes_do_not_create_release(self):
        for changed in (False, True):
            if changed:
                self.write('docs/guide.md', 'docs\n')
                self.commit('docs: revise guide')
            before = self.snapshot()
            release.prepare(self.root)
            self.assertEqual(self.snapshot(), before)

    def test_reverted_binding_change_does_not_create_release(self):
        self.write('zpl-node/index.cjs', '// change\n')
        self.commit('feat(node): temporary change')
        self.git('revert', '--no-edit', 'HEAD')
        before = self.snapshot()
        release.prepare(self.root)
        self.assertEqual(self.snapshot(), before)

    def test_rerun_and_completed_release_do_not_bump_again(self):
        self.write('zpl-node/index.cjs', '// change\n')
        self.commit('fix(node): repair wrapper')
        release.prepare(self.root)
        before = self.snapshot()
        release.prepare(self.root)
        self.assertEqual(self.snapshot(), before)
        self.commit('chore: prepare release')
        self.git('tag', 'zpl-v0.2.2')
        release.prepare(self.root)
        self.assertEqual(self.snapshot(), before)

    def native_release(self, new):
        release.stamp(self.root, new)
        self.write('zpl/CHANGELOG.md', f'# Changelog\n\n## [Unreleased]\n\n## [{new}] - 2026-10-09\n\n### Fixed\n\n- Native change.\n\n## [0.2.1] - 2026-10-09\n\n- Previous release.\n')

    def test_larger_native_bump_preserved_and_native_only_synchronizes(self):
        for bindings in (False, True):
            if bindings:
                self.write('zpl-node/index.cjs', '// repair\n')
                self.commit('fix(node): repair')
            self.native_release('0.4.0')
            release.prepare(self.root)
            self.assert_versions('0.4.0')
            self.assertIn('- Native change.', (self.root / 'zpl/CHANGELOG.md').read_text())

    def test_binding_break_raises_native_patch_and_preserves_notes(self):
        self.write('zpl-node/index.cjs', '// remove API\n')
        self.commit('feat(node)!: remove old API')
        self.native_release('0.2.2')
        release.prepare(self.root)
        self.assert_versions('0.3.0')
        notes = (self.root / 'zpl/CHANGELOG.md').read_text()
        self.assertIn('- Native change.', notes)
        self.assertIn('zpl-v0.2.1...zpl-v0.3.0', notes)
        self.assertNotIn('## [0.2.2]', notes)

    def test_old_binding_notes_are_not_rewritten(self):
        path = self.root / 'zpl/CHANGELOG.md'
        path.write_text(path.read_text() + '\n' + release.START + '\n- Older note\n' + release.END + '\n')
        self.commit('docs: add previous binding notes')
        self.write('zpl-node/index.cjs', '// repair\n')
        self.commit('fix(node): repair')
        release.prepare(self.root)
        notes = path.read_text()
        self.assertEqual(notes.count(release.START), 2)
        self.assertIn('- Older note', notes)

    def test_missing_or_mismatched_tag_fails_without_edits(self):
        self.git('tag', '-d', 'zpl-v0.2.1')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'No reachable'):
            release.prepare(self.root)
        self.git('tag', 'zpl-v0.2.9')
        with self.assertRaisesRegex(ValueError, 'does not match'):
            release.prepare(self.root)
        self.assertEqual(self.snapshot(), before)

    def test_unreachable_and_prerelease_tags_are_ignored(self):
        self.git('checkout', '-b', 'other')
        self.write('zpl/src/lib.rs', '// other branch\n')
        self.commit('feat!: unrelated')
        self.git('tag', 'zpl-v9.0.0')
        self.git('checkout', 'main')
        self.git('tag', 'zpl-v10.0.0-alpha.1')
        self.assertEqual(release.baseline(self.root), ((0, 2, 1), 'zpl-v0.2.1'))

    def test_bad_lock_does_not_partially_stamp(self):
        self.write('Cargo.lock', 'version = 4\n')
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'lock entry'):
            release.stamp(self.root, '0.2.2')
        self.assertEqual(self.snapshot(), before)

    def test_pr_body_contains_native_and_binding_notes(self):
        self.write('zpl-c/src/lib.rs', '// extra API\n')
        self.commit('feat(c): add API')
        release.prepare(self.root)
        body = release.pr_body(self.root)
        self.assertIn('zpl: 0.2.1 → 0.2.2', body)
        self.assertIn('feat(c): add API', body)

    def test_native_notes_do_not_duplicate_shared_binding_pr(self):
        self.write('zpl-node/index.cjs', '// shared change\n')
        self.commit('feat(node): shared change (#123)')
        self.native_release('0.2.2')
        path = self.root / 'zpl/CHANGELOG.md'
        path.write_text(path.read_text().replace('- Native change.', '- Native change ([#123](https://github.com/codyps/zpl/pull/123)).'))
        release.prepare(self.root)
        self.assertNotIn(release.START, path.read_text())
        before = self.snapshot()
        release.prepare(self.root)
        self.assertEqual(self.snapshot(), before)

    @unittest.skipUnless(os.environ.get('RELEASE_PLZ_BIN'), 'Set RELEASE_PLZ_BIN for pinned binary integration test')
    def test_real_native_break_and_binding_fix_keep_larger_bump(self):
        registry = Path(self.temp.name) / 'published'
        shutil.copytree(self.root, registry)
        self.write('zpl/src/lib.rs', 'pub fn changed_api() {}\n')
        self.commit('feat(render)!: change API')
        self.write('zpl-node/index.cjs', '// wrapper fix\n')
        self.commit('fix(node): repair wrapper')
        result = subprocess.run([os.environ['RELEASE_PLZ_BIN'], 'update', '--registry-manifest-path',
                                 str(registry / 'Cargo.toml')], cwd=self.root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        native = tomllib.loads((self.root / 'zpl/Cargo.toml').read_text())
        self.assertEqual(native['package']['version'], '0.3.0')
        release.prepare(self.root)
        self.assert_versions('0.3.0')
        notes = (self.root / 'zpl/CHANGELOG.md').read_text()
        self.assertIn('change API', notes)
        self.assertIn('repair wrapper', notes)

    @unittest.skipUnless(os.environ.get('RELEASE_PLZ_BIN'), 'Set RELEASE_PLZ_BIN for pinned binary integration test')
    def test_real_release_plz_then_binding_preparation(self):
        # Local released manifests avoid registry access; real release-plz still
        # compares packages/history, computes native versions, and updates Cargo.
        registry = Path(self.temp.name) / 'published'
        shutil.copytree(self.root, registry)
        self.write('zpl-node/index.cjs', '// wrapper fix\n')
        self.commit('fix(node): repair wrapper')
        result = subprocess.run([os.environ['RELEASE_PLZ_BIN'], 'update', '--registry-manifest-path',
                                 str(registry / 'Cargo.toml')], cwd=self.root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('already up to date', result.stderr + result.stdout)
        release.prepare(self.root)
        self.assert_versions('0.2.2')
        result = subprocess.run(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'],
                                cwd=self.root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
