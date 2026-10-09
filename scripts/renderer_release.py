"""Select a completed stable renderer release at the exact checkout.

GitHub release events created with GITHUB_TOKEN do not trigger other workflows:
https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow
"""

import json
import re
import subprocess
import tomllib


def release_version(root):
    version = tomllib.loads((root / "zpl/Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        return None
    tag = f"zpl-v{version}"
    tagged = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}^{{commit}}"],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if tagged.returncode == 1:
        return None
    tagged.check_returncode()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True)
    if tagged.stdout.strip() != head.strip():
        return None
    release = json.loads(
        subprocess.check_output(
            [
                "gh",
                "release",
                "view",
                tag,
                "--repo",
                "codyps/zpl",
                "--json",
                "isDraft,isPrerelease,tagName",
            ],
            cwd=root,
            text=True,
        )
    )
    if release["isDraft"] or release["isPrerelease"] or release["tagName"] != tag:
        raise ValueError(f"{tag} is not a completed stable GitHub release")
    return version
