"""Exercise actual Cargo builds: PR #28's unrelated version edit must be A/A."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from build import build_revision, digest, input_inventory
from common import comparison_kind


def command(args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True, stderr=subprocess.STDOUT).strip()


def write(root, path, contents):
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(contents)


class Builds(unittest.TestCase):
    def test_clean_normalized_builds_preserve_identical_code_and_detect_renderer_edits(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            checkout = root / 'checkout'
            checkout.mkdir()
            command(['git', 'init', '-q'], checkout)
            command(['git', 'config', 'user.name', 'Benchmark test'], checkout)
            command(['git', 'config', 'user.email', 'benchmark@example.invalid'], checkout)
            write(checkout, 'Cargo.toml', '[workspace]\nmembers = ["zpl", "zpl-cmd"]\nresolver = "2"\n'
                  '[workspace.package]\nlicense = "OSL-3.0"\n')
            write(checkout, 'zpl/Cargo.toml', '[package]\nname = "zpl"\nversion = "0.1.1"\nedition = "2021"\n'
                  'license.workspace = true\n')
            write(checkout, 'zpl/src/lib.rs', 'pub fn render() -> usize { 42 }\n')
            write(checkout, 'zpl-cmd/Cargo.toml', '[package]\nname = "zpl-cmd"\nversion = "0.1.0"\n'
                  '[dependencies]\nzpl = { version = "0.1.0", path = "../zpl" }\n')
            write(checkout, 'zpl-cmd/src/main.rs', 'fn main() {}\n')
            command(['cargo', 'generate-lockfile', '--offline'], checkout)

            def commit():
                command(['git', 'add', '.'], checkout)
                command(['git', 'commit', '-qm', 'test: prepare benchmark input'], checkout)

            commit()
            original = command(['git', 'rev-parse', 'HEAD'], checkout)
            # Distinct checkout paths are essential to reproduce the original
            # bug, even when all sources and compiler flags are identical.
            baseline = root / 'baseline'
            command(['git', 'clone', '-q', str(checkout), str(baseline)], root)
            build = root / 'build'
            harness = b'fn main() { println!("{}", std::hint::black_box(zpl::render())); }\n'
            _, binary, before = build_revision(baseline, build, root / 'base', harness)
            self.assertEqual(command([str(binary)], root), '42')
            (build / 'target/stale-output').write_text('must not survive the next build')

            path = checkout / 'zpl-cmd/Cargo.toml'
            path.write_text(path.read_text().replace('version = "0.1.0", path', 'version = "0.1.1", path'))
            commit()
            sha, binary, after = build_revision(checkout, build, root / 'head', harness)
            self.assertNotEqual(sha, original)
            self.assertFalse((build / 'target/stale-output').exists())
            self.assertEqual(before, after)
            self.assertEqual(comparison_kind({'builds': {'base': before, 'head': after}}), 'unchanged')
            self.assertEqual(command([str(binary)], root), '42')
            self.assertEqual(digest(binary.read_bytes()), after['binary_sha256'])
            self.assertEqual(digest((root / 'head/inputs.json').read_bytes()), after['inputs_sha256'])

            write(checkout, 'zpl/src/lib.rs', 'pub fn render() -> usize { 43 }\n')
            # Refuse to claim a commit identity for uncommitted source changes.
            with self.assertRaisesRegex(ValueError, 'tracked edits'):
                build_revision(checkout, build, root / 'dirty', harness)
            commit()
            _, binary, changed = build_revision(checkout, build, root / 'changed', harness)
            self.assertNotEqual(before['inputs_sha256'], changed['inputs_sha256'])
            self.assertNotEqual(before['binary_sha256'], changed['binary_sha256'])
            self.assertEqual(comparison_kind({'builds': {'base': before, 'head': changed}}), 'measured')
            self.assertEqual(command([str(binary)], root), '43')

    def test_inventory_tracks_packages_assets_versions_lock_and_features(self):
        with tempfile.TemporaryDirectory() as temp:
            build = Path(temp)
            inputs = ('harness/Cargo.toml', 'harness/Cargo.lock', 'harness/src/main.rs', 'source/Cargo.toml',
                      'source/zpl/Cargo.toml', 'source/zpl/src/lib.rs', 'source/zpl/assets/font.zbf',
                      'source/raster-diff/src/lib.rs', 'source/raster-diff/Cargo.toml')
            for path in inputs:
                write(build, path, 'original')
            write(build, 'source/zpl-cmd/Cargo.toml', 'unrelated version')
            metadata = dict(packages=[dict(source=None, manifest_path=str(build / path))
                                      for path in ('harness/Cargo.toml', 'source/zpl/Cargo.toml',
                                                   'source/raster-diff/Cargo.toml')],
                            resolve=dict(nodes=[dict(id='zpl', features=['default'])]))
            original = input_inventory(build, metadata)
            for path in inputs:
                write(build, path, 'changed')
                with self.subTest(path=path):
                    self.assertNotEqual(input_inventory(build, metadata), original)
                write(build, path, 'original')
            write(build, 'source/zpl-cmd/Cargo.toml', 'new unrelated version')
            self.assertEqual(input_inventory(build, metadata), original)
            changed = copy.deepcopy(metadata)
            changed['resolve']['nodes'][0]['features'].append('new-feature')
            self.assertNotEqual(input_inventory(build, changed), original)
            self.assertNotIn(str(build), json.dumps(original))


if __name__ == '__main__':
    unittest.main()
