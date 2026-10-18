#!/usr/bin/env python3
"""Build Apple static slices and stage the SwiftPM distribution.

XCFramework static libraries and SwiftPM binary targets:
https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle
https://developer.apple.com/documentation/xcode/distributing-binary-frameworks-as-swift-packages
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {
    'aarch64-apple-darwin': 'macosx',
    'x86_64-apple-darwin': 'macosx',
    'aarch64-apple-ios': 'iphoneos',
    'aarch64-apple-ios-sim': 'iphonesimulator',
    'x86_64-apple-ios': 'iphonesimulator',
}
SLICES = {
    'macos-arm64_x86_64': ('macos', None, ['arm64', 'x86_64']),
    'ios-arm64': ('ios', None, ['arm64']),
    'ios-arm64_x86_64-simulator': ('ios', 'simulator', ['arm64', 'x86_64']),
}
LOCAL_TARGET = '.binaryTarget(name: "CZpl", path: "CZpl.xcframework")'


def stable(version):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('Expected a stable renderer version')
    return version


def filename(version):
    return f'CZpl-{stable(version)}.xcframework.zip'


def run(*args, **kwargs):
    subprocess.run(list(map(str, args)), check=True, **kwargs)


def verify(path):
    """Validate architecture/platform metadata and matching, nonempty headers/libs."""
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)) or any(
            name.startswith('/') or '..' in Path(name).parts or '\\' in name for name in names
        ):
            raise ValueError('Invalid XCFramework archive paths')
        info = plistlib.loads(archive.read('CZpl.xcframework/Info.plist'))
        libraries = info.get('AvailableLibraries', [])
        if len(libraries) != len(SLICES) or info.get('XCFrameworkFormatVersion') != '1.0':
            raise ValueError('Incomplete XCFramework')
        found = set()
        headers = None
        for library in libraries:
            identifier = library['LibraryIdentifier']
            if identifier in found or identifier not in SLICES:
                raise ValueError('Unexpected XCFramework slice')
            found.add(identifier)
            platform, variant, architectures = SLICES[identifier]
            if (library['SupportedPlatform'] != platform or
                library.get('SupportedPlatformVariant') != variant or
                sorted(library['SupportedArchitectures']) != architectures or
                library['LibraryPath'] != 'libCZpl.a' or library['HeadersPath'] != 'Headers'):
                raise ValueError('Incorrect XCFramework slice metadata')
            prefix = f'CZpl.xcframework/{identifier}'
            if not archive.read(f'{prefix}/libCZpl.a').startswith(b'!<arch>\n') and not archive.read(
                f'{prefix}/libCZpl.a').startswith(bytes.fromhex('cafebabe')):
                raise ValueError('Expected a static or universal archive')
            current = {name: archive.read(f'{prefix}/Headers/{name}')
                       for name in ('zpl.h', 'zpl_config.h', 'module.modulemap')}
            if any(not value for value in current.values()) or (headers is not None and current != headers):
                raise ValueError('XCFramework headers differ across slices')
            headers = current
        if headers['module.modulemap'] != modulemap().encode():
            raise ValueError('Unexpected Clang module definition')
    return hashlib.sha256(path.read_bytes()).hexdigest()


def modulemap():
    return 'module CZpl {\n    header "zpl.h"\n    export *\n}\n'


def build(output):
    output = output.resolve()
    if output.exists():
        raise ValueError('Use a new output directory for each XCFramework build')
    output.mkdir(parents=True)
    run('python3', ROOT / 'zpl-c/generate-config.py', '--check')
    run('python3', ROOT / 'zpl-swift/generate-config.py', '--check')
    target_dir = output / 'cargo'
    for target, sdk in TARGETS.items():
        env = os.environ.copy()
        env.update(CARGO_TARGET_DIR=str(target_dir), MACOSX_DEPLOYMENT_TARGET='11.0',
                   IPHONEOS_DEPLOYMENT_TARGET='13.0')
        env['SDKROOT'] = subprocess.check_output(['xcrun', '--sdk', sdk, '--show-sdk-path'], text=True).strip()
        run('cargo', 'build', '--locked', '--release', '-p', 'zpl-c', '--target', target, cwd=ROOT, env=env)
    headers = output / 'Headers'
    shutil.copytree(ROOT / 'zpl-c/include', headers)
    (headers / 'module.modulemap').write_text(modulemap())
    groups = [list(TARGETS)[:2], ['aarch64-apple-ios'], list(TARGETS)[3:]]
    command = ['xcodebuild', '-create-xcframework']
    for identifier, targets in zip(SLICES, groups):
        library = output / identifier / 'libCZpl.a'
        library.parent.mkdir()
        run('xcrun', 'lipo', '-create', *[target_dir / t / 'release/libzpl_c.a' for t in targets], '-output', library)
        command += ['-library', str(library), '-headers', str(headers)]
    framework = output / 'CZpl.xcframework'
    run(*command, '-output', framework)
    version = tomllib.loads((ROOT / 'zpl/Cargo.toml').read_text())['package']['version']
    destination = output / filename(version)
    # Fixed ZIP timestamps/order/permissions make archive-only reruns reproducible.
    with zipfile.ZipFile(destination, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(framework.rglob('*')):
            if path.is_file():
                info = zipfile.ZipInfo(str(path.relative_to(output)), date_time=(2020, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())
    verify(destination)
    print(destination)


def stage(version, artifact, destination, local=False):
    stable(version)
    for crate in ('zpl', 'zpl-c'):
        if tomllib.loads((ROOT / crate / 'Cargo.toml').read_text())['package']['version'] != version:
            raise ValueError('Swift artifact, C binding and renderer versions must match')
    checksum = verify(artifact)
    if artifact.name != filename(version):
        raise ValueError('Incorrect XCFramework archive name')
    if destination.exists():
        raise ValueError('Stage into a new directory')
    destination.mkdir(parents=True)
    for directory in ('Sources', 'Tests/ZPLTests'):
        shutil.copytree(ROOT / 'zpl-swift' / directory, destination / directory)
    for name in ('README.md', 'LICENSE'):
        source = ROOT / 'LICENSE' if name == 'LICENSE' else ROOT / 'zpl-swift/README.md'
        shutil.copyfile(source, destination / name)
    text = (ROOT / 'zpl-swift/Package.swift').read_text()
    if text.count(LOCAL_TARGET) != 1:
        raise ValueError('Expected one local Swift binary target')
    if local:
        with zipfile.ZipFile(artifact) as archive:
            archive.extractall(destination)
    else:
        url = f'https://github.com/codyps/zpl/releases/download/zpl-v{version}/{artifact.name}'
        text = text.replace(LOCAL_TARGET, f'.binaryTarget(name: "CZpl", url: "{url}", checksum: "{checksum}")')
    (destination / 'Package.swift').write_text(text)
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    metadata = {'version': version, 'sourceCommit': revision, 'sha256': checksum, 'artifact': artifact.name}
    (destination / 'release.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('build')
    command.add_argument('output', type=Path)
    command = commands.add_parser('verify')
    command.add_argument('artifact', type=Path)
    command = commands.add_parser('stage')
    command.add_argument('version')
    command.add_argument('artifact', type=Path)
    command.add_argument('destination', type=Path)
    command.add_argument('--local', action='store_true')
    args = parser.parse_args()
    if args.command == 'build':
        build(args.output)
    elif args.command == 'verify':
        print(verify(args.artifact))
    else:
        stage(args.version, args.artifact, args.destination, args.local)
