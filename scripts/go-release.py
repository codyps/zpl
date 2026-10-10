#!/usr/bin/env python3
"""Publish the Go submodule tag and register it with the public Go proxy.

Subdirectory tags: https://go.dev/ref/mod#vcs-version
Proxy registration: https://go.dev/doc/modules/publishing#publishing-steps
Only call publish after the checked-out package has passed release validation.
"""

import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile

from renderer_release import release_version

MODULE = "github.com/codyps/zpl/zpl-go"
REPO = "https://github.com/codyps/zpl.git"


def selected(root):
    version = release_version(root)
    if version is None:
        return None
    # v2+ needs an intentional module/import-path migration, not +incompatible.
    if int(version.split(".")[0]) >= 2:
        raise ValueError("Go releases at v2+ require a module path migration")
    manifest = (root / "zpl-go/go.mod").read_text()
    if re.findall(r"(?m)^module\s+(\S+)\s*$", manifest) != [MODULE]:
        raise ValueError("Unexpected Go module path")
    return version


def prepare(root):
    version = selected(root)
    print(f"released={'true' if version else 'false'}")
    if version:
        print(f"version={version}")


def remote_commit(root, tag):
    ref = f"refs/tags/{tag}"
    output = subprocess.check_output(
        ["git", "ls-remote", REPO, ref, ref + "^{}"],
        cwd=root, text=True, timeout=60,
    )
    refs = {name: sha for sha, name in (line.split() for line in output.splitlines())}
    return refs.get(ref + "^{}", refs.get(ref))


def publish(root):
    version = selected(root)
    if version is None:
        raise ValueError("No completed renderer release at HEAD")
    # A tag publishes committed files, never local build/stamping changes.
    if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"],
                               cwd=root, text=True).strip():
        raise ValueError("Go publication requires a clean checkout")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    tag = f"zpl-go/v{version}"
    existing = remote_commit(root, tag)
    if existing is not None and existing != head:
        raise ValueError(f"Refusing to move existing {tag} from {existing} to {head}")
    if existing is None:
        # Use the job-scoped GitHub token without persisting git credentials.
        # Never PATCH/force a ref: concurrent creation fails safely.
        subprocess.run(["gh", "api", "repos/codyps/zpl/git/refs", "--method", "POST",
                        "-f", f"ref=refs/tags/{tag}", "-f", f"sha={head}"],
                       cwd=root, check=True, timeout=60)
    if remote_commit(root, tag) != head:
        raise ValueError(f"Remote {tag} does not match the renderer release")
    # A fresh cache and no direct/private fallback ensure the public proxy sees
    # the version even on a retry after the tag was created successfully.
    with tempfile.TemporaryDirectory(prefix="zpl-go-release-") as directory:
        env = os.environ | {
            "GOPROXY": "https://proxy.golang.org", "GOPRIVATE": "",
            "GONOPROXY": "none", "GONOSUMDB": "none", "GOSUMDB": "sum.golang.org",
            "GOWORK": "off", "GOMODCACHE": str(Path(directory) / "cache"),
        }
        subprocess.run(["go", "mod", "download", f"{MODULE}@v{version}"],
                       cwd=directory, env=env, check=True, timeout=180)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "publish"])
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    {"prepare": prepare, "publish": publish}[args.command](root)
