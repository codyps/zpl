#!/usr/bin/env python3
"""Check the pinned external fixtures without running any ZPL or foreign code.

Usage: check-zpl-corpus.py BINARYKITS_CHECKOUT TOOLCHAIN_CHECKOUT ZPLR_CHECKOUT
First build: direnv exec . cargo build -p zpl --example zpl-parse
"""
from pathlib import Path
import hashlib
import subprocess
import sys

if len(sys.argv) != 4:
    raise SystemExit(__doc__)
root = Path(__file__).resolve().parent.parent
checkouts = dict(zip(
    ["BinaryKits/BinaryKits.Zpl", "trevordcampbell/zpl-toolchain", "le2ni/zplr"],
    map(Path, sys.argv[1:]),
))
files = []
for row in (root / "docs/zpl-corpus-manifest.tsv").read_text().splitlines():
    if row.startswith("#"):
        continue
    repo, commit, name, length, digest = row.split("\t")
    file = checkouts[repo] / name
    data = file.read_bytes()
    if len(data) != int(length) or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"Fixture differs from pinned {commit}: {file}")
    files.append(str(file))
subprocess.run([str(root / "target/debug/examples/zpl-parse"), *files], check=True)
print(f"Verified and parsed {len(files)} pinned fixture files")
