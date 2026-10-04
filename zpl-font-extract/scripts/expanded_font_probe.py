"""Predeclare a broad sample campaign, then capture each bounded batch serially.

Existing experiments remain excluded. New square, stretch and rotation families
each reserve every fifth configuration for final validation, before any new
pixels are collected. This is a sample design, not adaptive fitting.
"""

import argparse
import json
from pathlib import Path

from capture_font_probe import sha
import reconstruction_probe as probe
from reconstruct_font import require, save


def configurations(previous):
    excluded, provenance = set(), []
    for root in previous:
        data = (root / "manifest.json").read_bytes()
        manifest = json.loads(data)
        provenance.append(dict(capture=root.name, manifest_sha256=sha(data)))
        for page in manifest["pages"]:
            if page.get("font", "0") != "0":
                continue
            for p in page["probes"]:
                excluded.add((p["width"] or p["height"], p["height"], p["orientation"]))
    families = {
        "square": {(n, n, "N") for n in range(8, 121)},
        "stretch": {
            (w, h, "N")
            for h in (
                10,
                12,
                14,
                16,
                18,
                20,
                24,
                28,
                32,
                40,
                48,
                56,
                64,
                72,
                80,
                88,
                96,
            )
            for w in (
                h - 2,
                h - 1,
                h + 1,
                h + 2,
                round(h * 3 / 4),
                round(h * 4 / 3),
                h // 2,
                h * 2,
            )
        },
        "rotation": {
            (n, n, r)
            for n in (10, 12, 16, 18, 20, 24, 28, 32, 40, 48, 56, 64, 72, 80, 88, 96)
            for r in "RIB"
        },
    }
    result = []
    seen = set(excluded)
    for family, candidates in families.items():
        for index, q in enumerate(
            sorted(candidates - seen, key=lambda q: (max(q[:2]), q))
        ):
            result.append(
                dict(
                    family=family,
                    query=q,
                    group="validation" if index % 5 == 2 else "development",
                )
            )
            seen.add(q)
    for group, sizes in (
        ("development", (128, 192, 352, 480)),
        ("validation", (144, 208, 304, 400, 464)),
    ):
        for n in sizes:
            q = (n, n, "N")
            require(q not in seen, "large sampling plan overlaps previous data")
            result.append(dict(family="large", query=q, group=group))
            seen.add(q)
    return result, provenance


def prepare(root, previous):
    rows, provenance = configurations(previous)
    root.mkdir(parents=True, exist_ok=False)
    campaigns = []
    for group in ("development", "validation"):
        for low, high in (
            (0, 32),
            (32, 64),
            (64, 96),
            (96, 128),
            (128, 192),
            (192, 512),
        ):
            selected = [
                r
                for r in rows
                if r["group"] == group
                and low < max(r["query"][:2]) <= high
                and r["family"] != "large"
            ]
            for start in range(0, len(selected), 24):
                chunk = selected[start : start + 24]
                name = f"{group}-{len(campaigns):02}"
                probe.prepare(root / name, "HOSgj@", [r["query"] for r in chunk], group)
                campaigns.append(
                    dict(name=name, group=group, role="small", configurations=chunk)
                )
        chunk = [r for r in rows if r["group"] == group and r["family"] == "large"]
        name = f"{group}-large"
        probe.prepare(root / name, "HOSgj@", [r["query"] for r in chunk], group)
        campaigns.append(
            dict(name=name, group=group, role="large", configurations=chunk)
        )
    for campaign in campaigns:
        manifest = (root / campaign["name"] / "manifest.json").read_bytes()
        campaign["manifest_sha256"] = sha(manifest)
        campaign["previews"] = len(json.loads(manifest)["pages"]) + 3
    save(
        root / "plan.json",
        dict(
            schema="expanded-font-sampling-v1",
            previous=provenance,
            campaigns=campaigns,
            configurations=len(rows),
            cases=len(rows) * 6,
            previews=sum(c["previews"] for c in campaigns),
        ),
    )


def capture(root, group, host):
    plan = json.loads((root / "plan.json").read_text())
    require(plan["schema"] == "expanded-font-sampling-v1", "unknown sampling plan")
    for campaign in plan["campaigns"]:
        if campaign["group"] != group:
            continue
        directory = root / campaign["name"]
        require(
            sha((directory / "manifest.json").read_bytes())
            == campaign["manifest_sha256"],
            "sampling plan changed",
        )
        # A failed campaign is not retried automatically. Completed campaigns can
        # be skipped when continuing a partially completed multi-batch collection.
        record = directory / "zd621/capture.json"
        if record.exists():
            require(
                json.loads(record.read_text())["status"] == "complete",
                "previous failed campaign requires diagnosis",
            )
            print(campaign["name"], "already complete", flush=True)
            continue
        print(campaign["name"], "starting", flush=True)
        probe.capture(directory, host, schema="small-font-probe-v1")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "capture"))
    parser.add_argument("root", type=Path)
    parser.add_argument("--previous", nargs="+", type=Path)
    parser.add_argument(
        "--group", choices=("development", "validation"), default="development"
    )
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    args = parser.parse_args()
    if args.action == "prepare":
        require(bool(args.previous), "supply previous campaigns to exclude")
        prepare(args.root, args.previous)
    else:
        capture(args.root, args.group, args.host)


if __name__ == "__main__":
    main()
