#!/usr/bin/env python3
"""Regenerate the shipped Wasm and Go translation; --check only verifies them.
Regenerate on Linux, matching CI: Rust embeds host-specific path separators
in panic locations even after prefix remapping. Requires the full Rust
workspace, wasm32-unknown-unknown, Go and patch.
Consumers use the checked-in artifacts and do not run this script.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VERSION = "v0.4.16"
TAGS = "!backend_cgo && !backend_purego && !backend_wazero"
PATCH = HERE / "wasm2go-memory.patch"
parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
if os.name == "nt":
    raise SystemExit("Regenerate on Linux (for example WSL), matching Go package CI; "
                     "Rust source-path separators otherwise change the Wasm artifact")

env = os.environ.copy()
target = Path(env.get("CARGO_TARGET_DIR", ROOT / "target/go-package")).resolve()
env["CARGO_TARGET_DIR"] = str(target)
# Panic/source locations in dependencies otherwise embed the developer's Cargo
# cache path. Apply remapping to the whole dependency graph, not only the bridge.
# https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix-remap-source-names-in-output
cargo_home = Path(env.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
env.pop("RUSTFLAGS", None)
env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join([
    f"--remap-path-prefix={ROOT}=/workspace",
    f"--remap-path-prefix={cargo_home}=/cargo",
])

def run(cmd, cwd=ROOT):
    subprocess.run(cmd, cwd=cwd, env=env, check=True, timeout=600)

run(["cargo", "rustc", "--locked", "--release", "-p", "zpl-go-prototype",
     "--target", "wasm32-unknown-unknown", "--lib", "--", "-C",
     "link-arg=--max-memory=268435456"])
# Resolve the pinned tool independently of any unrelated wasm2go on PATH.
with tempfile.TemporaryDirectory(prefix="zpl-go-generate-", dir=target) as directory:
    stage = Path(directory)
    # Go verifies the pinned module through its normal module checksum rules.
    # Patch a writable copy of the translator, never the shared module cache.
    # https://go.dev/ref/mod#go-mod-download
    module = json.loads(subprocess.check_output(
        ["go", "mod", "download", "-json", "github.com/ncruces/wasm2go@" + VERSION],
        cwd=stage, env=env, text=True, timeout=600,
    ))
    tool_source = stage / "wasm2go-source"
    shutil.copytree(module["Dir"], tool_source)
    for path in [tool_source, *tool_source.rglob("*")]:
        if not path.is_symlink():
            path.chmod(0o755 if path.is_dir() else 0o644)
    # No fuzzy application: a changed upstream helper template must be reviewed.
    run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(PATCH)], cwd=tool_source)
    run(["go", "build", "-buildvcs=false", "-o", str(stage / "wasm2go"), "."], cwd=tool_source)
    shutil.copyfile(target / "wasm32-unknown-unknown/release/zpl_go_prototype.wasm", stage / "guest.wasm")
    (stage / "internal/translated").mkdir(parents=True)
    run([str(stage / "wasm2go"), "-embed", "-pkg", "translated", "-tags", TAGS,
         "-o", "internal/translated/module.go", "guest.wasm"], cwd=stage)
    files = ["guest.wasm"] + [p.relative_to(stage).as_posix() for p in sorted((stage / "internal/translated").iterdir())]
    metadata = {
        "wasm2go": VERSION,
        "wasm2go_patch": {
            "file": PATCH.name,
            "sha256": hashlib.sha256(PATCH.read_bytes()).hexdigest(),
        },
        "rustc": subprocess.check_output(["rustc", "--version"], env=env, text=True).strip(),
        "target": "wasm32-unknown-unknown",
        "max_memory_bytes": 268435456,
        "translation_flags": ["-embed", "-pkg", "translated", "-tags", TAGS],
        "sha256": {name: hashlib.sha256((stage / name).read_bytes()).hexdigest() for name in files},
    }
    (stage / "generated.json").write_bytes((json.dumps(metadata, indent=2) + "\n").encode("utf-8"))
    files.append("generated.json")
    for name in files:
        source, destination = stage / name, HERE / name
        if args.check:
            if not destination.exists() or source.read_bytes() != destination.read_bytes():
                raise SystemExit(f"Generated artifact differs: {name}; regenerate and review")
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
    print("Verified" if args.check else "Generated", ", ".join(files))
