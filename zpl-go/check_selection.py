#!/usr/bin/env python3
"""Check all backend-tag combinations without compiling unrelated packages.
The package includes the guest/generated source. Positive configurations
are also executed by the shared Go contract tests; these checks focus on builds.
"""
import itertools
import os
from pathlib import Path
import subprocess

here = Path(__file__).resolve().parent
backends = ["cgo", "purego", "wazero", "wasm2go"]
checks = 0

def build(names, *, cgo=None, target=None, diagnostic=None):
    global checks
    tags = ",".join("backend_" + name for name in names)
    env = os.environ | {"CGO_ENABLED": cgo or ("1" if names == ("cgo",) else "0")}
    if target:
        env |= {"GOOS": target, "GOARCH": "amd64"}
    result = subprocess.run(
        ["go", "build", "-tags", tags, "."], cwd=here, env=env,
        capture_output=True, text=True, timeout=180,
    )
    if diagnostic:
        assert result.returncode != 0 and diagnostic in result.stderr, (
            tags, result.stdout, result.stderr
        )
    else:
        assert result.returncode == 0, (tags, result.stdout, result.stderr)
    checks += 1

for count in range(5):
    for names in itertools.combinations(backends, count):
        build(names, diagnostic=None if count <= 1 else
              "select_exactly_one_of_backend_cgo_backend_purego_backend_wazero_backend_wasm2go")
build((), cgo="1")
build(("cgo",), cgo="0", diagnostic="backend_cgo_requires_CGO_ENABLED_1")
build(("purego",), target="windows", diagnostic="backend_purego_prototype_requires_linux_or_darwin")
print(f"Passed {checks} backend-selection build checks")
