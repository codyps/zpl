#!/usr/bin/env python3
"""Stamp release versions and select unpublished PyPI files without uploading.

PyPI release metadata: https://docs.pypi.org/api/json/#get-a-release
Publication itself uses the PyPA trusted-publishing action in release-plz.yml.
"""

import argparse
import hashlib
import json
import re
import shutil
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

from renderer_release import release_version

PACKAGE = "zplkit"
CRATE = "zpl-python"


def stamp(root, version):
    """Update only the binding version and its existing workspace lock entry."""
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable renderer version")
    renderer = tomllib.loads((root / "zpl/Cargo.toml").read_text())["package"]
    if renderer["version"] != version:
        raise ValueError("Python release version does not match the renderer")
    project = tomllib.loads((root / "zpl-python/pyproject.toml").read_text())["project"]
    if project["name"] != PACKAGE or "version" not in project.get("dynamic", []):
        raise ValueError("Unexpected Python package name or version configuration")
    manifest_path = root / "zpl-python/Cargo.toml"
    manifest = manifest_path.read_text()
    package = tomllib.loads(manifest)["package"]
    if package["name"] != CRATE:
        raise ValueError("Unexpected binding crate name")
    old = re.escape(package["version"])
    manifest, count = re.subn(
        rf'(?m)^version = "{old}"$', f'version = "{version}"', manifest
    )
    if count != 1:
        raise ValueError("Expected exactly one binding package version")
    lock_path = root / "Cargo.lock"
    lock, count = re.subn(
        rf'(\[\[package\]\]\nname = "zpl-python"\nversion = "){old}("\n)',
        lambda m: f"{m[1]}{version}{m[2]}",
        lock_path.read_text(),
    )
    if count != 1:
        raise ValueError("Expected one matching binding entry in workspace Cargo.lock")
    # Validate both files before writing either; never create a crate-local lockfile.
    manifest_path.write_text(manifest)
    lock_path.write_text(lock)


def prepare(root):
    version = release_version(root)
    if version is None:
        print("released=false")
        return
    stamp(root, version)
    print("released=true")
    print(f"version={version}")


def registry_files(version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Expected a stable version")
    request = urllib.request.Request(
        f"https://pypi.org/pypi/{PACKAGE}/{version}/json",
        headers={"User-Agent": "codyps-zpl-release"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return {}
        raise
    if data["info"]["name"] != PACKAGE or data["info"]["version"] != version:
        raise ValueError("Unexpected PyPI release metadata")
    return {file["filename"]: file for file in data["urls"]}


def select_uploads(source, destination, version):
    """Require a complete build set; skip only byte-identical published files.

    Re-running a failed publish downloads the original workflow artifacts. A
    different file under an immutable published filename must fail visibly.
    """
    artifacts = sorted(source.iterdir())
    wheels = [
        p
        for p in artifacts
        if p.name.startswith(f"zplkit-{version}-") and p.suffix == ".whl"
    ]
    sdists = [p for p in artifacts if p.name == f"zplkit-{version}.tar.gz"]
    if len(wheels) != 5 or len(sdists) != 1 or len(artifacts) != 6:
        raise ValueError(
            "Expected five platform wheels and one source archive for this version"
        )
    if destination.exists() and any(destination.iterdir()):
        raise ValueError("Upload directory must be empty")
    published = registry_files(version)
    pending = []
    for artifact in artifacts:
        remote = published.get(artifact.name)
        if remote is None:
            pending.append(artifact)
        elif (
            remote["yanked"]
            or remote["digests"]["sha256"]
            != hashlib.sha256(artifact.read_bytes()).hexdigest()
        ):
            raise ValueError(
                f"Published file is yanked or has different content: {artifact.name}"
            )
    destination.mkdir(parents=True, exist_ok=True)
    for artifact in pending:
        shutil.copyfile(artifact, destination / artifact.name)
    print(f"publish={'true' if pending else 'false'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("prepare")
    stamp_args = commands.add_parser("stamp")
    stamp_args.add_argument("version")
    upload = commands.add_parser("select-uploads")
    upload.add_argument("version")
    upload.add_argument("source", type=Path)
    upload.add_argument("destination", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if args.command == "prepare":
        prepare(root)
    elif args.command == "stamp":
        stamp(root, args.version)
    else:
        select_uploads(args.source, args.destination, args.version)


if __name__ == "__main__":
    main()
