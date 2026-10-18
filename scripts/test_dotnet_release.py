"""NuGet release gating, package completeness, and immutable retry contracts."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import zipfile

spec = importlib.util.spec_from_file_location('dotnet_release', Path(__file__).with_name('dotnet-release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class DotnetReleaseTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.project = self.root / release.PROJECT
        self.project.parent.mkdir(parents=True)
        self.project.write_text('<Project><PropertyGroup><PackageId>Codyps.Zpl</PackageId><Version>1.2.3</Version></PropertyGroup></Project>')
        for crate in ('zpl', 'zpl-c'):
            (self.root / crate).mkdir()
            (self.root / crate / 'Cargo.toml').write_text('[package]\nversion = "1.2.3"\n')

    def archive(self, missing=(), changed=None, signature=False):
        files = {f'runtimes/{rid}/native/{name}': b'native' for rid, name in release.NATIVES.items()}
        files.update({
            **{path: b'managed' for path in release.MANAGED_ASSETS},
            'README.md': b'readme', 'LICENSE': b'license',
            'Codyps.Zpl.nuspec': b'<package><metadata><id>Codyps.Zpl</id><version>1.2.3</version><license>OSL-3.0</license></metadata></package>',
            '_rels/.rels': b'relationships', 'package/services/core-properties/id.psmdcp': b'metadata',
        })
        if signature:
            files['.signature.p7s'] = b'registry signature'
            files['_rels/.rels'] = b'different packaging id'
        files.update(changed or {})
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w') as archive:
            for name, data in files.items():
                if name not in missing:
                    archive.writestr(name, data)
        return buffer.getvalue()

    def test_gate_skips_ordinary_commits(self):
        before = self.project.read_bytes()
        with patch.object(release, 'release_version', return_value=None), contextlib.redirect_stdout(io.StringIO()) as output:
            release.prepare(self.root)
        self.assertEqual(output.getvalue(), 'released=false\n')
        self.assertEqual(self.project.read_bytes(), before)

    def test_completed_release_requires_all_versions_to_match(self):
        with patch.object(release, 'release_version', return_value='1.2.3'), contextlib.redirect_stdout(io.StringIO()) as output:
            release.prepare(self.root)
        self.assertEqual(output.getvalue(), 'released=true\nversion=1.2.3\n')
        self.project.write_text(self.project.read_text().replace('1.2.3', '1.2.2'))
        with patch.object(release, 'release_version', return_value='1.2.3'), self.assertRaisesRegex(ValueError, 'must match'):
            release.prepare(self.root)

    def test_stamp_is_validated_and_does_not_write(self):
        before = self.project.read_bytes()
        self.assertIn('<Version>2.0.0</Version>', release.stamp_text(self.root, '2.0.0'))
        self.assertEqual(self.project.read_bytes(), before)
        for invalid in ('01.2.3', '1.2.3-beta', '../x', '1.2'):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                release.stamp_text(self.root, invalid)

    def test_every_native_is_required(self):
        for rid, name in release.NATIVES.items():
            with self.subTest(rid=rid), self.assertRaisesRegex(ValueError, 'Incomplete'):
                release.payload(self.archive(missing=[f'runtimes/{rid}/native/{name}']), '1.2.3')

    def test_managed_compile_runtime_and_license_are_required(self):
        for path in sorted(release.MANAGED_ASSETS | {'LICENSE'}):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, 'Incomplete'):
                release.payload(self.archive(missing=[path]), '1.2.3')

    def test_bad_metadata_empty_and_unexpected_native_fail(self):
        for changes in ({'runtimes/win-arm64/native/zpl_c.dll': b'extra'},
                        {'ref/net8.0/Codyps.Zpl.dll': b''},
                        {'Codyps.Zpl.nuspec': b'<package><metadata><id>Other</id></metadata></package>'}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                release.payload(self.archive(changed=changes), '1.2.3')

    def test_local_single_rid_package_cannot_be_released(self):
        missing = [f'runtimes/{rid}/native/{name}' for rid, name in release.NATIVES.items() if rid != 'linux-x64']
        data = self.archive(missing=missing)
        release.payload(data, '1.2.3', ['linux-x64'])
        with self.assertRaises(ValueError):
            release.payload(data, '1.2.3')

    def test_new_and_identical_signed_release_retries(self):
        artifact = self.root / 'Codyps.Zpl.1.2.3.nupkg'
        artifact.write_bytes(self.archive())
        for remote, expected in ((None, 'true'), (self.archive(signature=True), 'false')):
            with patch.object(release, 'registry_package', return_value=remote), contextlib.redirect_stdout(io.StringIO()) as output:
                release.select_upload(artifact, '1.2.3')
            self.assertEqual(output.getvalue(), f'publish={expected}\n')

    def test_changed_immutable_payload_fails(self):
        artifact = self.root / 'Codyps.Zpl.1.2.3.nupkg'
        artifact.write_bytes(self.archive())
        with patch.object(release, 'registry_package', return_value=self.archive(changed={'runtimes/win-x64/native/zpl_c.dll': b'changed'})), self.assertRaisesRegex(ValueError, 'different content'):
            release.select_upload(artifact, '1.2.3')

    def test_only_registry_404_is_treated_as_unpublished(self):
        for status in (404, 401, 429, 500):
            error = urllib.error.HTTPError('url', status, 'failure', {}, None)
            with patch.object(release.urllib.request, 'urlopen', side_effect=error):
                if status == 404:
                    self.assertIsNone(release.registry_package('1.2.3'))
                else:
                    with self.assertRaises(urllib.error.HTTPError):
                        release.registry_package('1.2.3')


if __name__ == '__main__':
    unittest.main()
