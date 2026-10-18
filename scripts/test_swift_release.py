"""Swift artifact/immutable publication regressions; no remote writes."""
import importlib
import io
import json
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

package = importlib.import_module('swift-package')
release = importlib.import_module('swift-release')


def archive_bytes(change=None):
    stream = io.BytesIO()
    libraries = []
    with zipfile.ZipFile(stream, 'w') as archive:
        for identifier, (platform, variant, architectures) in package.SLICES.items():
            library = {'LibraryIdentifier': identifier, 'LibraryPath': 'libCZpl.a', 'HeadersPath': 'Headers',
                       'SupportedPlatform': platform, 'SupportedArchitectures': architectures}
            if variant:
                library['SupportedPlatformVariant'] = variant
            libraries.append(library)
            prefix = f'CZpl.xcframework/{identifier}'
            archive.writestr(f'{prefix}/libCZpl.a', b'!<arch>\nfixture')
            for name in ('zpl.h', 'zpl_config.h', 'module.modulemap'):
                text = package.modulemap() if name == 'module.modulemap' else name
                archive.writestr(f'{prefix}/Headers/{name}', text)
        if change:
            change(libraries)
        archive.writestr('CZpl.xcframework/Info.plist', plistlib.dumps({
            'XCFrameworkFormatVersion': '1.0', 'AvailableLibraries': libraries}))
    return stream.getvalue()


class SwiftReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.artifact = self.root / 'CZpl-1.2.3.xcframework.zip'
        self.artifact.write_bytes(archive_bytes())

    def test_complete_artifact_checksum(self):
        self.assertEqual(len(package.verify(self.artifact)), 64)

    def test_incomplete_or_wrong_platform_artifacts_fail(self):
        changes = [lambda ls: ls.pop(), lambda ls: ls[1].update(SupportedPlatform='macos'),
                   lambda ls: ls[2].update(SupportedArchitectures=['arm64']),
                   lambda ls: ls[1].update(SupportedPlatformVariant='simulator'),
                   lambda ls: ls.append(ls[0])]
        for change in changes:
            with self.subTest(change=change):
                self.artifact.write_bytes(archive_bytes(change))
                with self.assertRaises(ValueError):
                    package.verify(self.artifact)

    def test_invalid_versions_fail(self):
        for version in ('v1.2.3', '1.2', '1.2.3-beta.1', '01.2.3', '../1.2.3'):
            with self.assertRaises(ValueError):
                package.filename(version)

    def test_stage_binds_manifest_and_metadata_to_exact_zip_and_source(self):
        (self.root / 'zpl').mkdir()
        (self.root / 'zpl-c').mkdir()
        for crate in ('zpl', 'zpl-c'):
            (self.root / crate / 'Cargo.toml').write_text('[package]\nversion = "1.2.3"\n')
        (self.root / 'LICENSE').write_text('license')
        swift = self.root / 'zpl-swift'
        (swift / 'Sources').mkdir(parents=True)
        (swift / 'Tests/ZPLTests').mkdir(parents=True)
        (swift / 'README.md').write_text('usage')
        (swift / 'Sources/ZPL.swift').write_text('public enum ZPL {}')
        (swift / 'Package.swift').write_text(package.LOCAL_TARGET)
        destination = self.root / 'dist/zpl-swift'
        with patch.object(package, 'ROOT', self.root), patch.object(package.subprocess, 'check_output', return_value='abc\n'):
            package.stage('1.2.3', self.artifact, destination)
        manifest = (destination / 'Package.swift').read_text()
        self.assertIn('https://github.com/codyps/zpl/releases/download/zpl-v1.2.3/', manifest)
        self.assertIn(package.verify(self.artifact), manifest)
        self.assertNotIn('path:', manifest)
        self.assertEqual(json.loads((destination / 'release.json').read_text())['sourceCommit'], 'abc')
        with patch.object(package, 'ROOT', self.root), patch.object(release, 'ROOT', self.root), \
             patch.object(package.subprocess, 'check_output', return_value='abc\n'):
            release.validate('1.2.3', self.artifact, destination)
            (destination / 'Sources/ZPL.swift').write_text('tampered')
            with self.assertRaisesRegex(ValueError, 'differs'):
                release.validate('1.2.3', self.artifact, destination)

    def test_publication_requires_exact_completed_release(self):
        with patch.object(release, 'release_version', return_value=None), patch.object(release, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'exact completed'):
                release.publish('1.2.3', self.artifact, self.root)
            run.assert_not_called()

    def test_existing_binary_asset_must_match_before_any_upload(self):
        metadata = json.dumps({'tagName': 'zpl-v1.2.3', 'isDraft': False, 'isPrerelease': False,
                               'assets': [{'name': self.artifact.name}]})
        calls = []
        def download(*args, **kwargs):
            calls.append(args)
            Path(args[-1], self.artifact.name).write_bytes(b'different archive')
        with patch.object(release, 'output', return_value=metadata), patch.object(release, 'run', side_effect=download):
            with self.assertRaisesRegex(ValueError, 'never overwrite'):
                release.upload_asset('1.2.3', self.artifact)
        self.assertEqual(len(calls), 1)
        self.assertIn('download', calls[0])

    def test_tag_conflict_fails_before_upload(self):
        clone = self.root / 'remote'
        clone.mkdir()
        (clone / 'Package.swift').write_text('old')
        def command(*args, **kwargs):
            if args[:3] == ('git', 'tag', '--list'):
                return '1.2.3'
            if args[:3] == ('git', 'ls-files', '-z'):
                return 'Package.swift\0'
            return ''
        def action(*args, **kwargs):
            if 'clone' in args:
                target = Path(args[-1])
                target.mkdir()
                (target / 'Package.swift').write_text('old')
        desired = self.root / 'desired'
        desired.mkdir()
        (desired / 'Package.swift').write_text('new')
        with patch.object(release, 'release_version', return_value='1.2.3'), \
             patch.object(release, 'validate'), patch.object(release, 'output', side_effect=command), \
             patch.object(release, 'run', side_effect=action), patch.object(release, 'upload_asset') as upload:
            with self.assertRaisesRegex(ValueError, 'never move'):
                release.publish('1.2.3', self.artifact, desired)
            upload.assert_not_called()


if __name__ == '__main__':
    unittest.main()
