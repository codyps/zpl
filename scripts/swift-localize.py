#!/usr/bin/env python3
"""Restore the exact checksummed CI binaries for an offline SwiftPM consumer."""
import argparse
import importlib
import json
from pathlib import Path
import zipfile


def localize(package, artifacts):
    apple = importlib.import_module('swift-package')
    native = importlib.import_module('swift-native')
    metadata = json.loads((package / 'release.json').read_text())
    version = metadata['version']
    text = (package / 'Package.swift').read_text()
    pending = []
    for helper, name, checksum in ((apple, metadata['artifact'], metadata['sha256']),
                                    (native, metadata['nativeArtifact'], metadata['nativeSha256'])):
        artifact = artifacts / helper.filename(version)
        if name != artifact.name or helper.verify(artifact) != checksum:
            raise ValueError('Staged binary checksum does not match the tested Swift distribution')
        target = 'CZpl' if helper is apple else 'CZplNative'
        url = f'https://github.com/codyps/zpl/releases/download/zpl-v{version}/{artifact.name}'
        remote = f'.binaryTarget(name: "{target}", url: "{url}", checksum: "{checksum}")'
        if text.count(remote) != 1:
            raise ValueError('Unexpected staged Swift binary target')
        text = text.replace(remote, helper.LOCAL_TARGET)
        pending.append(artifact)
    for artifact in pending:
        with zipfile.ZipFile(artifact) as archive:
            archive.extractall(package)
    (package / 'Package.swift').write_text(text)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('package', type=Path)
    parser.add_argument('artifacts', type=Path)
    args = parser.parse_args()
    localize(args.package, args.artifacts)
