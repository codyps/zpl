#!/usr/bin/env python3
"""Package SwiftPM static C library variants for Linux and Windows (SE-0482).

https://github.com/swiftlang/swift-evolution/blob/main/proposals/0482-swiftpm-static-library-binary-target-non-apple-platforms.md
Use CZpl.lib, without a lib prefix, for SwiftPM's Windows library-name handling:
https://github.com/swiftlang/swift-package-manager/blob/swift-6.3.3-RELEASE/Sources/Build/BuildPlan/BuildPlan%2BProduct.swift
"""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import zipfile

apple = importlib.import_module('swift-package')
BUNDLE = 'CZpl.artifactbundle'
TARGETS = {
    'x86_64-unknown-linux-gnu': ['x86_64-unknown-linux-gnu'],
    'aarch64-unknown-linux-gnu': ['aarch64-unknown-linux-gnu'],
    'x86_64-unknown-linux-musl': ['x86_64-unknown-linux-musl', 'x86_64-swift-linux-musl'],
    'aarch64-unknown-linux-musl': ['aarch64-unknown-linux-musl', 'aarch64-swift-linux-musl'],
    'x86_64-pc-windows-msvc': ['x86_64-unknown-windows-msvc', 'x86_64-pc-windows-msvc'],
    'aarch64-pc-windows-msvc': ['aarch64-unknown-windows-msvc', 'aarch64-pc-windows-msvc'],
}
LOCAL_TARGET = '.binaryTarget(name: "CZplNative", path: "CZpl.artifactbundle")'


def filename(version):
    return f'CZpl-{apple.stable(version)}.artifactbundle.zip'


def library(target):
    return f'{target}/' + ('CZpl.lib' if 'windows' in target else 'libCZpl.a')


def manifest(version, targets):
    return {'schemaVersion': '1.0', 'artifacts': {'CZplNative': {
        'type': 'staticLibrary', 'version': apple.stable(version),
        'variants': [{'path': library(target), 'supportedTriples': TARGETS[target],
                      'staticLibraryMetadata': {'headerPaths': ['include'],
                                                'moduleMapPath': 'include/module.modulemap'}}
                     for target in targets]}}}


def verify(path, version=None, targets=None):
    targets = list(TARGETS if targets is None else targets)
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        metadata = json.loads(archive.read(f'{BUNDLE}/info.json'))
        if version is None:
            version = metadata['artifacts']['CZplNative']['version']
        if metadata != manifest(version, targets):
            raise ValueError('Incomplete or incorrect Swift native platform metadata')
        expected = {f'{BUNDLE}/info.json'} | {f'{BUNDLE}/{library(t)}' for t in targets}
        expected |= {f'{BUNDLE}/include/{name}' for name in ('zpl.h', 'zpl_config.h', 'module.modulemap')}
        if len(names) != len(set(names)) or set(names) != expected:
            raise ValueError('Unexpected Swift artifact paths or missing native libraries')
        for target in targets:
            if not archive.read(f'{BUNDLE}/{library(target)}').startswith(b'!<arch>\n'):
                raise ValueError('Expected a native static library')
        for name in ('zpl.h', 'zpl_config.h'):
            if not archive.read(f'{BUNDLE}/include/{name}'):
                raise ValueError('Missing native C headers')
        if archive.read(f'{BUNDLE}/include/module.modulemap') != apple.modulemap().encode():
            raise ValueError('Unexpected native Clang module definition')
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pack(version, libraries, output, targets=None):
    targets = list(TARGETS if targets is None else targets)
    if output.name != filename(version):
        raise ValueError('Unexpected native artifact filename')
    files = {'info.json': (json.dumps(manifest(version, targets), indent=2) + '\n').encode()}
    for target in targets:
        files[library(target)] = (libraries / library(target)).read_bytes()
    for name in ('zpl.h', 'zpl_config.h'):
        files[f'include/{name}'] = (apple.ROOT / 'zpl-c/include' / name).read_bytes()
    files['include/module.modulemap'] = apple.modulemap().encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(f'{BUNDLE}/{name}', date_time=(2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    verify(output, version, targets)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('pack')
    command.add_argument('version')
    command.add_argument('libraries', type=Path)
    command.add_argument('output', type=Path)
    command = commands.add_parser('verify')
    command.add_argument('artifact', type=Path)
    args = parser.parse_args()
    if args.command == 'pack':
        pack(args.version, args.libraries, args.output)
    else:
        print(verify(args.artifact))
