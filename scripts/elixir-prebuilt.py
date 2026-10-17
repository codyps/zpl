#!/usr/bin/env python3
"""Validate upstream NIF archives, generate checksums, and publish immutable assets.

CI builds/archives NIFs with philss/rustler-precompiled-action. Checksums use
the upstream Mix task against a loopback mirror of those exact CI artifacts:
https://hexdocs.pm/rustler_precompiled/precompilation_guide.html
NIF 2.15 is Rustler 0.38's default and runs on our minimum OTP 25 and newer.
"""

import argparse
import functools
import http.server
import json
import os
import re
import subprocess
import tarfile
import tempfile
import threading
from pathlib import Path

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


def artifacts(version, directory, targets=TARGETS):
    expected = {filename(version, target) for target in targets}
    found = {p.name for p in directory.glob("*.tar.gz")}
    if found != expected:
        raise ValueError(
            f"Prebuilt artifact set mismatch: missing={expected - found}, extra={found - expected}"
        )
    paths = [directory / name for name in sorted(expected)]
    for path in paths:
        with tarfile.open(path) as archive:
            members = archive.getmembers()
            if (
                len(members) != 1
                or not members[0].isfile()
                or members[0].name != path.name.removesuffix(".tar.gz")
            ):
                raise ValueError(f"Invalid NIF archive: {path.name}")
    return paths


def checksums(version, directory, package, targets=TARGETS):
    artifacts(version, directory, targets)
    if targets != TARGETS and len(targets) != 1:
        raise ValueError("Generate checksums for every target or just the native target")
    handler = functools.partial(
        http.server.SimpleHTTPRequestHandler, directory=str(directory.resolve())
    )
    # The upstream task downloads before hashing; nothing must be published to
    # GitHub until these artifacts and the resulting Hex archive pass CI.
    with (
        http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) as server,
        tempfile.TemporaryDirectory(prefix="zpl-checksums-") as tmp,
    ):
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            config = Path(tmp) / "config.exs"
            config.write_text(
                "import Config\n"
                f'config :zpl, :precompiled_base_url, "http://127.0.0.1:{server.server_port}/"\n'
            )
            env = os.environ.copy()
            env["MIX_ENV"] = "prod"
            # Bootstrap metadata by compiling the staged sources: there is no
            # checksum manifest yet. The download task then writes that manifest.
            env["ZPL_BUILD"] = "true"
            env["RUSTLER_PRECOMPILED_GLOBAL_CACHE_PATH"] = str(Path(tmp) / "cache")
            env["NO_PROXY"] = env.get("NO_PROXY", "") + ",127.0.0.1,localhost"
            env["no_proxy"] = env["NO_PROXY"]
            mix = "mix.bat" if os.name == "nt" else "mix"
            subprocess.run([mix, "deps.get"], cwd=package, env=env, check=True)
            subprocess.run(
                [
                    mix,
                    "do",
                    "loadconfig",
                    f"{config},",
                    "rustler_precompiled.download",
                    "Zpl.Native",
                    "--all" if targets == TARGETS else "--only-local",
                    "--print",
                ],
                cwd=package,
                env=env,
                check=True,
            )
        finally:
            server.shutdown()


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
    for name in ("checksums", "publish"):
        command = commands.add_parser(name)
        command.add_argument("version")
        command.add_argument("directory", type=Path)
        if name == "checksums":
            command.add_argument("package", type=Path)
            command.add_argument("--target", choices=TARGETS)
    args = parser.parse_args()
    if args.command == "checksums":
        checksums(
            args.version,
            args.directory,
            args.package,
            (args.target,) if args.target else TARGETS,
        )
    else:
        publish(args.version, args.directory)


if __name__ == "__main__":
    main()
