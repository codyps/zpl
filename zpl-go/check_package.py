#!/usr/bin/env python3
"""Test an isolated copy containing only distributable files, without ignored artifacts.
Go module archives omit symlinks; LICENSE must therefore be a regular file.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

here = Path(__file__).resolve().parent
root = here.parent
artifacts = here / "artifacts"
artifacts.mkdir(exist_ok=True)
files = subprocess.check_output(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "zpl-go"],
    cwd=root,
).decode().split("\0")
with tempfile.TemporaryDirectory(prefix="package-", dir=artifacts) as directory:
    dest = Path(directory)
    for name in set(files) - {""}:
        source = root / name
        if source.is_symlink() or not source.is_file():
            continue
        output = dest / source.relative_to(here)
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, output)
    assert (dest / "LICENSE").is_file(), "module archive needs a regular LICENSE"
    assert not (dest / "artifacts").exists(), "local artifacts leaked into package"
    env = os.environ | {"CGO_ENABLED": "0", "GOWORK": "off", "CC": "unavailable-cc"}
    for tags in [[], ["-tags", "backend_wazero"]]:
        subprocess.run(["go", "test", "-count=1", *tags, "./..."],
                       cwd=dest, env=env, check=True, timeout=600)
print("Standalone package passed: default wasm2go and wazero, no C compiler or ignored local artifacts")
