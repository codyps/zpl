"""Prevent build matrices and consumer target selection drifting from release gates."""
import re
from pathlib import Path
import unittest

from prebuilt_targets import DOTNET_NATIVES, ELIXIR_TARGETS, PYTHON_PLATFORMS

ROOT = Path(__file__).resolve().parent.parent


def matrix(package, job):
    text = (ROOT / f'.github/workflows/{package}-package.yml').read_text()
    text = text.split(f'\n  {job}:', 1)[1].split('    runs-on:', 1)[0]
    return [dict(re.findall(r'^            ([\w-]+): (.+)$', row, re.M))
            for row in text.split('          - runner:')[1:]]


class PlatformContractTests(unittest.TestCase):
    def test_elixir_download_selection_and_build_matrix_match_release_gate(self):
        source = (ROOT / 'zpl-elixir/lib/zpl/native.ex').read_text()
        targets = re.search(r'targets:\s*~w\(([^)]+)\)', source)[1].split()
        built = [row['target'] for row in matrix('elixir', 'native')]
        self.assertCountEqual(targets, ELIXIR_TARGETS)
        self.assertCountEqual(built, ELIXIR_TARGETS)

    def test_nuget_builds_and_tests_every_packaged_rid(self):
        for job in ('native', 'consumer'):
            self.assertCountEqual([row['rid'] for row in matrix('dotnet', job)], DOTNET_NATIVES)

    def test_python_builds_each_required_platform_with_target_consumers(self):
        platforms = []
        for row in matrix('python', 'wheels'):
            target = row['target']
            arch = target.split('-')[0]
            if 'linux' in target:
                arch = {'armv7': 'armv7l', 'riscv64gc': 'riscv64'}.get(arch, arch)
                platforms.append(f"{row['compatibility']}_{arch}")
                self.assertIn('image', row)
                self.assertIn('docker-platform', row)
                if 'musl' in target:
                    self.assertEqual(row['compatibility'], 'musllinux_1_2')
                    self.assertIn('alpine', row['image'])
            elif 'apple' in target:
                platforms.append('macosx_arm64' if arch == 'aarch64' else 'macosx_x86_64')
            else:
                platforms.append({'i686': 'win32', 'x86_64': 'win_amd64', 'aarch64': 'win_arm64'}[arch])
        self.assertCountEqual(platforms, PYTHON_PLATFORMS)


if __name__ == '__main__':
    unittest.main()
