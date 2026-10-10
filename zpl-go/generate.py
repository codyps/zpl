#!/usr/bin/env python3
"""Regenerate the shipped Wasm and Go translation; --check only verifies them.
Requires the full Rust workspace, wasm32-unknown-unknown, Go and pinned wasm2go.
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
parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
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
    env["GOBIN"] = str(stage)
    run(["go", "install", "github.com/ncruces/wasm2go@" + VERSION])
    shutil.copyfile(target / "wasm32-unknown-unknown/release/zpl_go_prototype.wasm", stage / "guest.wasm")
    (stage / "internal/translated").mkdir(parents=True)
    run([str(stage / "wasm2go"), "-embed", "-pkg", "translated", "-tags", TAGS,
         "-o", "internal/translated/module.go", "guest.wasm"], cwd=stage)
    files = ["guest.wasm"] + [str(p.relative_to(stage)) for p in sorted((stage / "internal/translated").iterdir())]
    metadata = {
        "wasm2go": VERSION,
        "rustc": subprocess.check_output(["rustc", "--version"], env=env, text=True).strip(),
        "target": "wasm32-unknown-unknown",
        "max_memory_bytes": 268435456,
        "translation_flags": ["-embed", "-pkg", "translated", "-tags", TAGS],
        "sha256": {name: hashlib.sha256((stage / name).read_bytes()).hexdigest() for name in files},
    }
    (stage / "generated.json").write_text(json.dumps(metadata, indent=2) + "\n")
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
