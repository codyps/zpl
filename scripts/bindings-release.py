#!/usr/bin/env python3
"""Complete release-plz update with binding changes; never publish or push.

Version policy follows the pinned release-plz defaults:
https://github.com/release-plz/release-plz/blob/release-plz-v0.3.169/crates/next_version/src/version_increment.rs
Breaking markers: https://www.conventionalcommits.org/en/v1.0.0/#specification
Run after `release-plz update` in a clean CI checkout with full history/tags.
"""

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
import tomllib

BINDINGS = ("zpl-c", "zpl-wasm", "zpl-node", "zpl-python", "zpl-elixir")
PATHS = (*BINDINGS, "scripts/build-node.sh", "scripts/elixir-package.py")
CRATES = {"zpl": "zpl", "zpl-c": "zpl-c", "zpl-wasm": "zpl-wasm",
          "zpl-python": "zpl-python", "zpl-elixir": "zpl_elixir"}
REPO = "https://github.com/codyps/zpl"
START = "<!-- binding-release-notes -->"
END = "<!-- /binding-release-notes -->"


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def version(value):
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value):
        raise ValueError(f"Expected a stable version, got {value!r}")
    return tuple(map(int, value.split(".")))


def format_version(value):
    return ".".join(map(str, value))


def baseline(root):
    # Ignore prereleases and tags on unrelated branches. Missing history is an
    # error, not permission to invent a first version or re-release old changes.
    tags = git(root, "tag", "--merged", "HEAD", "--list", "zpl-v*").splitlines()
    stable = [(version(t[5:]), t) for t in tags
              if re.fullmatch(r"zpl-v\d+\.\d+\.\d+", t)]
    if not stable:
        raise ValueError("No reachable stable renderer tag; fetch full release history")
    number, tag = max(stable)
    manifest = tomllib.loads(git(root, "show", f"{tag}:zpl/Cargo.toml"))
    if version(manifest["package"]["version"]) != number:
        raise ValueError(f"Renderer manifest does not match {tag}")
    return number, tag


def binding_commits(root, tag):
    changed = git(root, "diff", "--name-only", tag, "HEAD", "--", *PATHS)
    if not changed:
        return []
    # Full messages include BREAKING CHANGE footers. Include merge messages too:
    # repositories can merge normally as well as squash, and merge resolutions
    # may themselves change binding files. NULs cannot occur in commit messages.
    fields = git(root, "log", "--format=%H%x00%B%x00", f"{tag}..HEAD", "--", *PATHS).split("\0")
    commits = [(fields[i].strip(), fields[i + 1].strip())
               for i in range(0, len(fields) - 1, 2)]
    if not commits:
        raise ValueError("Binding diff has no commits; release history is incomplete")
    return commits


def next_binding_version(base, commits, config):
    if not commits:
        return base
    policy = dict(config.get("workspace", {}))
    policy.update(next(p for p in config["package"] if p["name"] == "zpl"))
    # Fail visibly if the Rust policy changes beyond the defaults mirrored here.
    for field in ("custom_major_increment_regex", "custom_minor_increment_regex", "release_commits"):
        if field in policy:
            raise ValueError(f"Binding version policy must be updated for {field}")
    messages = [message for _, message in commits]
    breaking = any(re.match(r"\w+(?:\([^\n]+\))?!: ", m) or
                   re.search(r"(?m)^BREAKING[ -]CHANGE: ", m) for m in messages)
    feature = any(re.match(r"feat(?:\([^\n]+\))?: ", m) for m in messages)
    major, minor, patch = base
    if breaking and major:
        return major + 1, 0, 0
    if (breaking and minor) or (feature and (major or policy.get("features_always_increment_minor", False))):
        return major, minor + 1, 0
    return major, minor, patch + 1


def replace_one(pattern, replacement, text, label):
    text, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"Expected one {label}, found {count}")
    return text


def stamp(root, target):
    """Validate every edit before writing; preserve unrelated dependency versions."""
    edits = {}
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    members = workspace["workspace"]["members"]
    if not set(CRATES).issubset(members):
        raise ValueError("Expected all binding crates in the workspace")
    for member in members:
        path = root / member / "Cargo.toml"
        text = path.read_text()
        manifest = tomllib.loads(text)
        if member in CRATES:
            if manifest["package"]["name"] != CRATES[member]:
                raise ValueError(f"Unexpected crate name in {path}")
            old = re.escape(manifest["package"]["version"])
            text = replace_one(rf'^version = "{old}"$', f'version = "{target}"', text, str(path))
        # Workspace renderer dependencies currently use this inline-table style.
        # Fail rather than silently omit a dependency after a manifest refactor.
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            dep = manifest.get(section, {}).get("zpl")
            if dep is not None:
                old = re.escape(dep["version"])
                text = replace_one(rf'^(zpl = \{{ version = "){old}(",)',
                                   lambda m: f'{m[1]}{target}{m[2]}', text, f'{path} zpl requirement')
        if text != path.read_text():
            edits[path] = text
    lock_path = root / "Cargo.lock"
    lock = lock_path.read_text()
    for crate in CRATES.values():
        lock = replace_one(rf'(\[\[package\]\]\nname = "{crate}"\nversion = ")[^"]+("\n)',
                           lambda m: f'{m[1]}{target}{m[2]}', lock, f'{crate} lock entry')
    edits[lock_path] = lock
    node_path = root / "zpl-node/package.json"
    node = node_path.read_text()
    if json.loads(node)["name"] != "@codyps/zpl":
        raise ValueError("Unexpected npm package")
    edits[node_path] = replace_one(r'("version":\s*")[^"]+(")',
                                  lambda m: f'{m[1]}{target}{m[2]}', node, "npm version")
    mix_path = root / "zpl-elixir/mix.exs"
    edits[mix_path] = replace_one(r'^(\s*version: ")[^"]+(",)$',
                                 lambda m: f'{m[1]}{target}{m[2]}', mix_path.read_text(), "Mix version")
    for path, text in edits.items():
        path.write_text(text)


def changelog(root, base, selected, target, commits):
    path = root / "zpl/CHANGELOG.md"
    text = path.read_text()
    # Edit only the pending release, leaving older binding note blocks intact.
    if selected > base:
        pattern = rf"(?ms)^## \[{re.escape(format_version(selected))}\][^\n]*\n(.*?)(?=^## |\Z)"
        match = re.search(pattern, text)
        if match is None:
            raise ValueError("Missing pending renderer release notes")
        body = match[1].strip()
        before, after = text[:match.start()], text[match.end():]
    else:
        marker = "## [Unreleased]\n"
        if text.count(marker) != 1:
            raise ValueError("Expected one Unreleased heading")
        before, after = text.split(marker)
        before += marker + "\n"
        body = ""
    if commits:
        # A commit can touch both the renderer and a binding. Do not repeat PRs
        # already present in release-plz's native notes. Strip our own block first
        # so repeated preparation does not accidentally remove binding notes.
        native = re.sub(re.escape(START) + r".*?" + re.escape(END), "", body, flags=re.DOTALL).strip()
        lines = []
        for sha, message in commits:
            subject = message.splitlines()[0]
            pull = re.search(r"\(#(\d+)\)$", subject)
            if pull and f"[#{pull[1]}]" in native:
                continue
            lines.append(f"- {subject} ([{sha[:7]}]({REPO}/commit/{sha}))")
        block = f"{START}\n### Bindings\n\n" + "\n".join(lines) + f"\n{END}"
        body = "\n\n".join(part for part in (block if lines else "", native) if part)
    text = before + release_heading(base, target) + "\n\n" + body + "\n\n" + after.lstrip("\n")
    path.write_text(text)


def release_heading(base, target):
    date = datetime.now(timezone.utc).date().isoformat()
    return (f"## [{format_version(target)}]({REPO}/compare/"
            f"zpl-v{format_version(base)}...zpl-v{format_version(target)}) - {date}")


def prepare(root):
    base, tag = baseline(root)
    selected = version(tomllib.loads((root / "zpl/Cargo.toml").read_text())["package"]["version"])
    if selected < base:
        raise ValueError("Renderer checkout is older than its latest reachable release")
    commits = binding_commits(root, tag)
    config = tomllib.loads((root / "release-plz.toml").read_text())
    target = max(selected, next_binding_version(base, commits, config))
    if target == base:
        return  # Avoid version-only churn or a PR immediately after publication.
    stamp(root, format_version(target))
    changelog(root, base, selected, target, commits)


def pr_body(root):
    notes = []
    for directory in ("raster-diff", "zpl-bitmap-fonts", "zpl"):
        path = f"{directory}/Cargo.toml"
        old = tomllib.loads(git(root, "show", f"HEAD:{path}"))["package"]["version"]
        new = tomllib.loads((root / path).read_text())["package"]["version"]
        if old != new:
            text = (root / directory / "CHANGELOG.md").read_text()
            section = re.search(rf"(?ms)^## \[{re.escape(new)}\][^\n]*\n(.*?)(?=^## |\Z)", text)
            if section is None:
                raise ValueError(f"Missing release notes for {directory} {new}")
            notes.append(f"### {directory}: {old} → {new}\n\n{section[1].strip()}")
    return ("Prepare the next release from release-plz's Rust analysis and binding changes.\n\n"
            "C, Wasm/Node, Python, and Elixir versions follow the renderer. "
            "Merging this PR runs the existing registry publishers; C remains source-only.\n\n"
            + "\n\n".join(notes) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--body", type=Path, required=True, help="PR body output outside the checkout")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    prepare(root)
    args.body.write_text(pr_body(root))
