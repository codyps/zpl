#!/usr/bin/env python3
"""Select only a completed stable zpl release at the checked-out commit.

Keep npm in the release-plz workflow: GITHUB_TOKEN-created releases do not
trigger another release workflow. https://docs.github.com/en/actions/how-tos/
write-workflows/choose-when-workflows-run/trigger-a-workflow
"""
import json
from pathlib import Path

from renderer_release import release_version


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
