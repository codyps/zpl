#!/usr/bin/env python3
"""Produce a compact, reviewable snapshot from run.py's raw artifacts."""
import hashlib
import json
from pathlib import Path
import statistics

here = Path(__file__).resolve().parent
raw = json.loads((here / "artifacts/report.json").read_text())
result = {
    "schema": 1,
    "host": {k: raw[k] for k in ("platform", "machine", "cpu", "gomaxprocs")},
    "base_commit": raw["commit"],
    "versions": {k: raw[k] for k in ("go", "rustc", "cargo")},
    "wasm2go_version": "v0.4.16",
    "measurement": raw["measurement"],
    "artifacts": {k: raw[k] for k in ("wasm_sha256", "wasm_bytes", "library_bytes", "generated_go_bytes")},
    "producer_build_seconds": {k: v["seconds"] for k, v in raw["builds"].items()},
    "backends": {},
}
for name, b in raw["backends"].items():
    samples = {}
    for process in b["bench_processes"]:
        assert process["failed"] == 0
        for case in process["results"]:
            samples.setdefault(case["Name"], []).extend(case["NS"])
    result["backends"][name] = {
        "test_exit": b["test"]["exit"],
        "oracle_cases": len(b["correctness"]["results"]),
        "oracle_failures": b["correctness"]["failed"],
        "binary_bytes": b["binary_bytes"],
        "consumer_clean_build_seconds": b["build"]["seconds"],
        "consumer_cached_build_seconds": b["rebuild"]["seconds"],
        "cold_process_ms": [c["seconds"] * 1000 for c in b["cold"]],
        "engine_init_ms": [c["init_ns"] / 1e6 for c in b["cold"]],
        "first_render_ms": [c["first_render_ns"] / 1e6 for c in b["cold"]],
        "warm": {
            name: {"ns_per_op_samples": values, "median_ms": statistics.median(values) / 1e6}
            for name, values in samples.items()
        },
    }
result["oracle"] = [
    {k: c[k] for k in ("Name", "Status", "OracleSHA256")}
    for c in json.loads((here / "artifacts/cases.json").read_text())
]
# Capture inputs as well as compiled guest; no large generated artifacts retained.
result["input_sha256"] = {
    p.name: hashlib.sha256(p.read_bytes()).hexdigest()
    for p in sorted((here / "artifacts/inputs").glob("*.zpl"))
}
cross = here / "artifacts/cross.json"
if cross.exists():
    result["cross_builds"] = json.loads(cross.read_text())
(here / "results").mkdir(exist_ok=True)
(here / "results/linux-amd64.json").write_text(json.dumps(result, indent=2) + "\n")
