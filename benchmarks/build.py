"""Build both revisions at one physical path and retain their provenance."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

METHOD = 'same-path-clean-v1'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def input_inventory(build, metadata):
    # Cargo metadata describes the standalone harness's resolved dependency
    # graph, including enabled features (not the original workspace's members).
    # https://doc.rust-lang.org/cargo/commands/cargo-metadata.html#output-format
    paths = {build / 'source/Cargo.toml', build / 'harness/Cargo.toml', build / 'harness/Cargo.lock'}
    for package in metadata['packages']:
        if package['source'] is not None:
            continue  # Registry/git contents are identified by the resolved lock.
        root = Path(package['manifest_path']).parent
        root.relative_to(build)  # Reject path dependencies outside the snapshot.
        if root == build / 'harness':
            paths.add(root / 'src/main.rs')
        else:
            # Conservatively include every local package file: Rust, embedded
            # assets, build scripts, manifests, and supporting data. Version
            # changes here can affect env!("CARGO_PKG_VERSION"); never strip them.
            paths.update(p for p in root.rglob('*') if p.is_file() or p.is_symlink())
            for parent in root.parents:
                if parent == build:
                    break
                if (parent / 'Cargo.toml').is_file():
                    paths.add(parent / 'Cargo.toml')
    files = {}
    for path in sorted(paths):
        path.resolve().relative_to(build)
        files[path.relative_to(build).as_posix()] = digest(path.read_bytes())
    # Paths must not make the inventory depend on the output directory's name.
    graph = json.loads(json.dumps(metadata).replace(str(build), '$BUILD'))
    return dict(files=files, cargo=graph)


def build_revision(checkout, build, saved, harness):
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=checkout, text=True).strip()
    if subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--'], cwd=checkout).returncode:
        raise ValueError('Benchmark checkouts must have no tracked edits; commit changes first')
    # Cargo hashes the physical path of out-of-workspace dependencies into crate
    # metadata. Separate baseline/candidate paths changed symbols and layout
    # even for identical sources: https://github.com/rust-lang/cargo/issues/7645
    # Recreate BOTH source and target at the same path. Cleaning prevents stale
    # outputs/mtimes from making an apparent A/A control reuse the first build.
    if build.exists():
        shutil.rmtree(build)
    source = build / 'source'
    source.mkdir(parents=True)
    with tempfile.TemporaryFile() as archive:
        subprocess.run(['git', 'archive', '--format=tar', commit], cwd=checkout,
                       stdout=archive, check=True)
        archive.seek(0)
        with tarfile.open(fileobj=archive) as snapshot:
            snapshot.extractall(source, filter='data')
    # Keep the harness beside the source workspace, not above it: Cargo must
    # resolve workspace-inherited package/dependency fields in source/Cargo.toml.
    project = build / 'harness'
    (project / 'src').mkdir(parents=True)
    (project / 'src/main.rs').write_bytes(harness)
    (project / 'Cargo.toml').write_text(
        '[package]\nname = "zpl-perf-harness"\nversion = "0.0.0"\nedition = "2021"\n'
        '[workspace]\n[dependencies]\nzpl = { path = "../source/zpl" }\n')
    shutil.copyfile(source / 'Cargo.lock', project / 'Cargo.lock')
    env = dict(os.environ, CARGO_TARGET_DIR=str(build / 'target'), CARGO_INCREMENTAL='0')
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--format-version=1'], cwd=project, env=env, text=True))
    original = tomllib.loads((source / 'Cargo.lock').read_text())
    resolved = tomllib.loads((project / 'Cargo.lock').read_text())
    locked = {(p['name'], p['version'], p.get('source'), p.get('checksum')) for p in original['package']}
    if any((p['name'], p['version'], p.get('source'), p.get('checksum')) not in locked
           for p in resolved['package'] if p['name'] != 'zpl-perf-harness'):
        raise ValueError('Harness changed dependency versions')
    inventory = encoded(input_inventory(build, metadata))
    saved.mkdir(parents=True)
    (saved / 'inputs.json').write_bytes(inventory)
    shutil.copyfile(project / 'Cargo.lock', saved / 'Cargo.lock')
    subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=project, env=env, check=True)
    binary = saved / 'zpl-perf-harness'
    shutil.copy2(build / 'target/release/zpl-perf-harness', binary)
    return commit, binary, dict(inputs_sha256=digest(inventory), binary_sha256=digest(binary.read_bytes()))
