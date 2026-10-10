#!/usr/bin/env python3
"""Package Rustler NIFs, pin their checksums, and upload immutable release assets.

Filename/checksum contract (including .so on macOS):
https://hexdocs.pm/rustler_precompiled/precompilation_guide.html
NIF 2.15 is Rustler 0.38's default and runs on our minimum OTP 25 and newer.
"""

import argparse
import gzip
import hashlib
import io
import json
import re
import subprocess
import tarfile
import tempfile
from pathlib import Path

import tomllib

TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
CHECKSUM = "checksum-Elixir.Zpl.Native.exs"


def filename(version, target):
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable package version")
    if target not in TARGETS:
        raise ValueError(f"Unsupported prebuilt target: {target}")
    prefix, extension = ("", "dll") if "windows" in target else ("lib", "so")
    return f"{prefix}zpl_elixir-v{version}-nif-2.15-{target}.{extension}.tar.gz"


def pack(version, target, library, destination):
    name = filename(version, target)
    data = library.read_bytes()
    destination.mkdir(parents=True, exist_ok=True)
    path = destination / name
    # Stable tar/gzip metadata makes retries independent of timestamps/paths.
    with (
        path.open("wb") as output,
        gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as zipped,
        tarfile.open(fileobj=zipped, mode="w") as archive,
    ):
        entry = tarfile.TarInfo(name.removesuffix(".tar.gz"))
        entry.mode = 0o755
        entry.size = len(data)
        archive.addfile(entry, io.BytesIO(data))
    return path


def artifacts(version, directory, targets=TARGETS):
    expected = {filename(version, target) for target in targets}
    found = {p.name for p in directory.glob("*.tar.gz")}
    if found != expected:
        raise ValueError(
            f"Prebuilt artifact set mismatch: missing={expected - found}, extra={found - expected}"
        )
    return [directory / name for name in sorted(expected)]


def checksums(version, directory, package, targets=TARGETS):
    paths = artifacts(version, directory, targets)
    entries = []
    for path in paths:
        with tarfile.open(path) as archive:
            members = archive.getmembers()
            if (
                len(members) != 1
                or not members[0].isfile()
                or members[0].name != path.name.removesuffix(".tar.gz")
            ):
                raise ValueError(f"Invalid NIF archive: {path.name}")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        entries.append(f'  "{path.name}" => "sha256:{digest}",\n')
    (package / CHECKSUM).write_text("%{\n" + "".join(entries) + "}\n")


def publish(version, directory):
    paths = artifacts(version, directory)
    tag = f"zpl-v{version}"
    common = ["--repo", "codyps/zpl"]
    metadata = json.loads(
        subprocess.check_output(
            [
                "gh",
                "release",
                "view",
                tag,
                *common,
                "--json",
                "tagName,isDraft,isPrerelease,assets",
            ],
            text=True,
        )
    )
    if metadata["tagName"] != tag or metadata["isDraft"] or metadata["isPrerelease"]:
        raise ValueError("Expected a completed stable renderer release")
    existing = {asset["name"] for asset in metadata["assets"]}
    missing = []
    # Compare all existing assets before uploading anything. Never overwrite.
    with tempfile.TemporaryDirectory() as tmp:
        for path in paths:
            if path.name not in existing:
                missing.append(path)
                continue
            subprocess.run(
                [
                    "gh",
                    "release",
                    "download",
                    tag,
                    *common,
                    "--pattern",
                    path.name,
                    "--dir",
                    tmp,
                ],
                check=True,
            )
            if (Path(tmp) / path.name).read_bytes() != path.read_bytes():
                raise ValueError(f"Existing release asset differs: {path.name}")
    if missing:
        subprocess.run(
            ["gh", "release", "upload", tag, *common, *map(str, missing)], check=True
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("pack")
    build.add_argument("target", choices=TARGETS)
    build.add_argument("library", type=Path)
    build.add_argument("destination", type=Path)
    for name in ("checksums", "publish"):
        command = commands.add_parser(name)
        command.add_argument("version")
        command.add_argument("directory", type=Path)
        if name == "checksums":
            command.add_argument("package", type=Path)
            command.add_argument("--target", choices=TARGETS, action="append")
    args = parser.parse_args()
    if args.command == "pack":
        root = Path(__file__).resolve().parent.parent
        version = tomllib.loads((root / "zpl-elixir/Cargo.toml").read_text())[
            "package"
        ]["version"]
        print(pack(version, args.target, args.library, args.destination))
    elif args.command == "checksums":
        checksums(args.version, args.directory, args.package, args.target or TARGETS)
    else:
        publish(args.version, args.directory)


if __name__ == "__main__":
    main()
