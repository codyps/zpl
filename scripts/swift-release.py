#!/usr/bin/env python3
"""Publish tested, immutable XCFramework assets and the Swift distribution tag.

The source release must already exist at this exact checkout. Distribution uses
ordinary semver Git tags, as required by SwiftPM:
https://docs.swift.org/main/documentation/packagemanagerdocs/addingdependencies/
"""
import argparse
import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

from renderer_release import release_version

package = importlib.import_module('swift-package')
ROOT = Path(__file__).resolve().parent.parent
DISTRIBUTION = 'codyps/zpl-swift'


def output(*args, **kwargs):
    return subprocess.check_output(list(map(str, args)), text=True, **kwargs).strip()


def run(*args, **kwargs):
    subprocess.run(list(map(str, args)), check=True, **kwargs)


def prepare():
    version = release_version(ROOT)
    if version is None:
        print('released=false')
        return
    native = tomllib.loads((ROOT / 'zpl-c/Cargo.toml').read_text())['package']['version']
    if native != version:
        raise ValueError('C binding and renderer versions must match')
    print(f'released=true\nversion={version}')


def files(directory):
    return {str(p.relative_to(directory)): p.read_bytes() for p in directory.rglob('*')
            if p.is_file() and '.git' not in p.relative_to(directory).parts
            and '.build' not in p.relative_to(directory).parts}


def validate(version, artifact, distribution):
    package.stable(version)
    if artifact.name != package.filename(version):
        raise ValueError('Unexpected Swift asset filename')
    metadata = json.loads((distribution / 'release.json').read_text())
    revision = output('git', 'rev-parse', 'HEAD', cwd=ROOT)
    if metadata != {'version': version, 'sourceCommit': revision,
                    'sha256': package.verify(artifact), 'artifact': artifact.name}:
        raise ValueError('Swift package metadata does not match tested artifact/source')
    # Compare the complete staged tree, including its manifest/checksum and source,
    # before trusting it for remote writes. No local binary targets may be published.
    with tempfile.TemporaryDirectory() as tmp:
        expected = Path(tmp) / 'zpl-swift'
        package.stage(version, artifact, expected)
        if files(expected) != files(distribution):
            raise ValueError('Swift distribution differs from the staged source package')


def upload_asset(version, artifact):
    tag = f'zpl-v{version}'
    release = json.loads(output('gh', 'release', 'view', tag, '--repo', 'codyps/zpl',
                                '--json', 'tagName,isDraft,isPrerelease,assets'))
    if release['tagName'] != tag or release['isDraft'] or release['isPrerelease']:
        raise ValueError('Expected a completed stable renderer release')
    if artifact.name in {asset['name'] for asset in release['assets']}:
        with tempfile.TemporaryDirectory() as tmp:
            run('gh', 'release', 'download', tag, '--repo', 'codyps/zpl', '--pattern', artifact.name, '--dir', tmp)
            if (Path(tmp) / artifact.name).read_bytes() != artifact.read_bytes():
                raise ValueError('Published Swift asset differs; never overwrite an existing version')
    else:
        run('gh', 'release', 'upload', tag, '--repo', 'codyps/zpl', artifact)


def publish(version, artifact, distribution):
    if release_version(ROOT) != version:
        raise ValueError('Publish only from the exact completed renderer release commit')
    validate(version, artifact, distribution)
    desired = files(distribution)
    # Use the configured gh identity without persisting credentials in Git config
    # or embedding a token in URLs, arguments, generated files, or commit history.
    git = ['git', '-c', 'credential.helper=', '-c', 'credential.helper=!gh auth git-credential']
    with tempfile.TemporaryDirectory() as tmp:
        clone = Path(tmp) / 'zpl-swift'
        run(*git, 'clone', f'https://github.com/{DISTRIBUTION}.git', clone)
        tags = output('git', 'tag', '--list', version, cwd=clone).splitlines()
        if tags:
            run('git', 'checkout', '--detach', version, cwd=clone)
            tracked = output('git', 'ls-files', '-z', cwd=clone).split('\0')
            actual = {name: (clone / name).read_bytes() for name in tracked if name}
            if actual != desired:
                raise ValueError('Published Swift tag has different contents; never move a published tag')
            upload_asset(version, artifact)
            return
        upload_asset(version, artifact)
        # Exercise the actual HTTPS binary URL and checksum after upload, before
        # making the new package version discoverable. Build/test jobs use no token.
        run('swift', 'test', '--package-path', distribution)
        with tempfile.TemporaryDirectory() as consumer_tmp:
            consumer = Path(consumer_tmp) / 'Consumer'
            shutil.copytree(ROOT / 'zpl-swift/Tests/Consumer', consumer, ignore=shutil.ignore_patterns('.build'))
            env = os.environ.copy()
            env.pop('ZPL_PACKAGE_URL', None)
            env.pop('ZPL_PACKAGE_VERSION', None)
            env['ZPL_PACKAGE_PATH'] = str(distribution.resolve())
            run('swift', 'run', '--package-path', consumer, cwd=consumer, env=env)
        # Replace only this dedicated generated repository's files. Never force
        # push: concurrent edits or a conflicting immutable tag fail visibly.
        tracked = output('git', 'ls-files', '-z', cwd=clone).split('\0')
        for name in tracked:
            if name:
                (clone / name).unlink()
        for name, data in desired.items():
            path = clone / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        run('git', 'add', '--all', cwd=clone)
        run('git', '-c', 'user.name=github-actions[bot]',
            '-c', 'user.email=41898282+github-actions[bot]@users.noreply.github.com',
            'commit', '-m', f'chore: publish Swift package {version}', cwd=clone)
        run('git', 'tag', version, cwd=clone)
        run(*git, 'push', '--atomic', 'origin', 'HEAD:refs/heads/main', f'refs/tags/{version}', cwd=clone)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('prepare')
    command = commands.add_parser('publish')
    command.add_argument('version')
    command.add_argument('artifact', type=Path)
    command.add_argument('distribution', type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare()
    else:
        publish(args.version, args.artifact, args.distribution)
