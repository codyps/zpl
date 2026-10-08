#!/usr/bin/env python3
"""Select only a completed stable zpl release at the checked-out commit.

Keep npm in the release-plz workflow: GITHUB_TOKEN-created releases do not
trigger another release workflow. https://docs.github.com/en/actions/how-tos/
write-workflows/choose-when-workflows-run/trigger-a-workflow
"""
import json
import re
import subprocess
import tomllib
from pathlib import Path


def release_version(root):
    version = tomllib.loads((root / "zpl/Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        return None
    tag = f"zpl-v{version}"
    tagged = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}^{{commit}}"],
        cwd=root, text=True, capture_output=True,
    )
    if tagged.returncode == 1:
        return None
    tagged.check_returncode()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True)
    if tagged.stdout.strip() != head.strip():
        return None
    release = json.loads(subprocess.check_output(
        ["gh", "release", "view", tag, "--repo", "codyps/zpl", "--json", "isDraft,isPrerelease,tagName"],
        cwd=root, text=True,
    ))
    if release["isDraft"] or release["isPrerelease"] or release["tagName"] != tag:
        raise ValueError(f"{tag} is not a completed stable GitHub release")
    return version


def prepare(root):
    version = release_version(root)
    if version is None:
        print("released=false")
        return
    package = root / "zpl-node/package.json"
    manifest = json.loads(package.read_text())
    if manifest["name"] != "@codyps/zpl":
        raise ValueError("Unexpected npm package name")
    manifest["version"] = version
    package.write_text(json.dumps(manifest, indent=2) + "\n")
    print("released=true")
    print(f"version={version}")


if __name__ == "__main__":
    prepare(Path(__file__).resolve().parent.parent)
