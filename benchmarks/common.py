"""Benchmark protocol and paired analysis (standard library only)."""
import math
import random
import statistics

NAMES = tuple(f"{case}/{stage}" for case in ("text", "barcodes", "graphics")
              for stage in ("scene", "raster", "total"))
RUNNERS = {"ubuntu-24.04"}

COMPARISON_NOTES = {
    'unchanged': 'No relevant input changes: the resolved package inputs and independently built '
                 'executables are identical. Timings are an A/A noise control; '
                 'no source performance change is reported.',
    'identical': 'The package inputs changed, but the independently built executables are identical. '
                 'Timings are an A/A noise control; no source performance change is reported.',
    'unreproducible': 'Inconclusive comparison: the inventoried package inputs match, but the '
                      'executables differ. Investigate untracked build inputs or nondeterminism '
                      'before attributing timing differences to this change.',
    'legacy': 'This run has no comparable-build provenance. Its timing differences cannot '
              'establish a source performance change; collect a run with the current tooling.',
}


def comparison_kind(record):
    if not record.get('builds'):
        return 'legacy'
    base, head = (record['builds'][label] for label in ('base', 'head'))
    same_inputs = base['inputs_sha256'] == head['inputs_sha256']
    if base['binary_sha256'] == head['binary_sha256']:
        return 'unchanged' if same_inputs else 'identical'
    return 'unreproducible' if same_inputs else 'measured'


def compare(before, after, config):
    if len(before) != 10 or len(after) != 10:
        raise ValueError("Expected ten paired rounds")
    if any(type(n) not in (int, float) or not math.isfinite(n) or n <= 0
           for n in [*before, *after]):
        raise ValueError("Expected positive finite timings")
    # Subtract first so an exact 10% improvement is not rounded just below 10%.
    changes = [(b - a) * 100 / a for a, b in zip(before, after)]
    rng = random.Random(0)
    draws = sorted(statistics.median(rng.choices(changes, k=10))
                   for _ in range(config["bootstrap_iterations"]))
    interval = [draws[int(.005 * (len(draws) - 1))],
                draws[math.ceil(.995 * (len(draws) - 1))]]
    percent = statistics.median(changes)
    delta = statistics.median([b - a for a, b in zip(before, after)])
    return {"percent": percent, "interval": interval,
            "alert": abs(percent) >= config["threshold_percent"]
            and abs(delta) >= config["threshold_ns"]
            and (interval[0] > 0 or interval[1] < 0)}
