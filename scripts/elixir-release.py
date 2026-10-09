#!/usr/bin/env python3
"""Stamp Hex versions and publish exactly the tested source archive.

Release API and outer SHA-256 checksum contract:
https://github.com/hexpm/hex/blob/v2.5.1/src/mix_hex_api_release.erl
https://hex.pm/docs/publish
No POST retries or replacement uploads: reruns check the published checksum first.
"""

import argparse
import hashlib
import io
import json
import os
import re
import tarfile
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

from renderer_release import release_version

PACKAGE = "zpl"
CRATE = "zpl_elixir"
API = "https://hex.pm/api"


def stable(version):
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable renderer version")


def stamp(root, version):
    stable(version)
    renderer = tomllib.loads((root / "zpl/Cargo.toml").read_text())["package"]
    if renderer["version"] != version:
        raise ValueError("Elixir version does not match the renderer")
    manifest_path = root / "zpl-elixir/Cargo.toml"
    manifest = manifest_path.read_text()
    package = tomllib.loads(manifest)["package"]
    if package["name"] != CRATE:
        raise ValueError("Unexpected binding crate name")
    old = re.escape(package["version"])
    manifest, count = re.subn(
        rf'(?m)^version = "{old}"$', f'version = "{version}"', manifest
    )
    if count != 1:
        raise ValueError("Expected exactly one binding version")
    mix_path = root / "zpl-elixir/mix.exs"
    mix = mix_path.read_text()
    if len(re.findall(r"\bapp: :zpl\b", mix)) != 1:
        raise ValueError("Unexpected Mix application")
    mix, count = re.subn(
        rf'(?m)^(\s*)version: "{old}",$', rf'\g<1>version: "{version}",', mix
    )
    if count != 1:
        raise ValueError("Expected one matching Mix version")
    lock_path = root / "Cargo.lock"
    lock, count = re.subn(
        rf'(\[\[package\]\]\nname = "{CRATE}"\nversion = "){old}("\n)',
        lambda m: f"{m[1]}{version}{m[2]}",
        lock_path.read_text(),
    )
    if count != 1:
        raise ValueError("Expected one binding entry in workspace Cargo.lock")
    # Validate all three before mutating any; never create a checkout-local lock.
    for path, data in [(manifest_path, manifest), (mix_path, mix), (lock_path, lock)]:
        path.write_text(data)


def prepare(root):
    version = release_version(root)
    if version is None:
        print("released=false")
        return
    stamp(root, version)
    print(f"released=true\nversion={version}")


def archive_bytes(path, version):
    stable(version)
    if path.name != f"{PACKAGE}-{version}.tar":
        raise ValueError("Unexpected Hex archive filename")
    data = path.read_bytes()
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        if archive.getnames().count("metadata.config") != 1:
            raise ValueError("Expected one Hex metadata file")
        metadata = archive.extractfile("metadata.config").read().decode()
    for key, expected in [("name", PACKAGE), ("app", PACKAGE), ("version", version)]:
        values = re.findall(r'\{<<"' + key + r'">>,\s*<<"([^"\n]+)">>\}\.', metadata)
        if values != [expected]:
            raise ValueError(f"Unexpected Hex {key} metadata")
    return data


def registry_release(version):
    stable(version)
    request = urllib.request.Request(
        f"{API}/packages/{PACKAGE}/releases/{version}",
        headers={"User-Agent": "codyps-zpl-release", "Accept": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def verify_release(metadata, version, checksum):
    if (
        metadata["version"] != version
        or metadata["checksum"] != checksum
        or metadata["retirement"] is not None
    ):
        raise ValueError("Hex version is retired or has different version/content")


def publish(path, version, *, check_only=False):
    data = archive_bytes(path, version)
    checksum = hashlib.sha256(data).hexdigest()
    existing = registry_release(version)
    if existing is not None:
        verify_release(existing, version, checksum)
        print("publish=false")
        return
    if check_only:
        print("publish=true")
        return
    key = os.environ.get("HEX_API_KEY")
    if not key:
        raise ValueError(
            "Configure the HEX_API_KEY repository secret before publishing"
        )
    request = urllib.request.Request(
        f"{API}/packages/{PACKAGE}/releases?replace=false",
        data=data,
        method="POST",
        headers={
            "Authorization": key,
            "Content-Type": "application/octet-stream",
            "Accept": "application/json",
            "User-Agent": "codyps-zpl-release",
        },
    )
    # Publish the exact tested bytes once. Never rebuild, replace, or retry a POST.
    with urllib.request.urlopen(request, timeout=60) as response:
        verify_release(json.load(response), version, checksum)
    print(f"Published {PACKAGE} {version} ({checksum})")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("prepare")
    stamping = commands.add_parser("stamp")
    stamping.add_argument("version")
    for command in ["check", "publish"]:
        upload = commands.add_parser(command)
        upload.add_argument("version")
        upload.add_argument("archive", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if args.command == "prepare":
        prepare(root)
    elif args.command == "stamp":
        stamp(root, args.version)
    else:
        publish(args.archive, args.version, check_only=args.command == "check")


if __name__ == "__main__":
    main()
