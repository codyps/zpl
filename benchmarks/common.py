"""Benchmark protocol and paired analysis (standard library only)."""
import math
import random
import statistics

NAMES = tuple(f"{case}/{stage}" for case in ("text", "barcodes", "graphics")
              for stage in ("scene", "raster", "total"))
RUNNERS = {"ubuntu-24.04"}


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
