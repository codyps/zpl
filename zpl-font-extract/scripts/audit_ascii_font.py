"""Freeze and audit full-ASCII candidates on native glyph and spacing holdouts.

No font selection or fitting occurs here. Freeze pins candidate bytes and planned
requests before validation capture. Each field and every native canvas retain
exact underpaint, overpaint and union counts. Blank space is checked by advance.
"""

import argparse
from datetime import datetime
import json
from pathlib import Path

from ascii_font_metrics import extract
from capture_font_probe import now
from fit_font_target import metrics
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha


def prepare(fit, sampling, output):
    require(not output.exists(), "audit plan already exists")
    plan = json.loads((sampling / "plan.json").read_text())
    fit_plan = json.loads((fit / "plan.json").read_text())
    report = json.loads((fit / "report.json").read_text())
    require(
        report["candidates_sha256"] == sha((fit / "candidates.json").read_bytes())
        and report["plan_sha256"] == sha((fit / "plan.json").read_bytes()),
        "fit inputs changed",
    )
    candidates = []
    for label in ("baseline", "targeted"):
        path = fit / (label + ".ttf")
        require(
            sha(path.read_bytes()) == report["variants"][label]["font_sha256"],
            "fit font changed",
        )
        candidates.append(
            dict(label=label, file=path.name, sha256=sha(path.read_bytes()))
        )
    captures = []
    forbidden = set(
        map(tuple, fit_plan["search_queries"] + fit_plan["internal_check_queries"])
    )
    for c in plan["campaigns"]:
        if c["group"] != "validation":
            continue
        root = sampling / c["name"]
        require(not (root / "zd621").exists(), "validation capture predates freeze")
        data = (root / "manifest.json").read_bytes()
        require(sha(data) == c["manifest_sha256"], "sampling plan changed")
        if c["role"] != "spacing":
            for page in json.loads(data)["pages"]:
                for p in page["probes"]:
                    q = (
                        p["width"],
                        p["height"],
                        ord(p["text"]),
                        "NRIB".index(p["orientation"]),
                    )
                    require(q not in forbidden, "validation overlaps fitting")
        captures.append(dict(name=c["name"], role=c["role"], manifest_sha256=sha(data)))
    save(
        output,
        dict(
            schema="ascii-font-audit-v1",
            frozen_at=now(),
            fit_plan_sha256=sha((fit / "plan.json").read_bytes()),
            fit_report_sha256=sha((fit / "report.json").read_bytes()),
            sampling_sha256=sha((sampling / "plan.json").read_bytes()),
            engine_sha256=fit_plan["engine_sha256"],
            printer=fit_plan["printer"],
            fonts=candidates,
            captures=captures,
        ),
    )


def text_pages(pages, rows):
    results = []
    for page, reference in pages:
        candidate = set()
        cases = []
        accounted = set()
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            turns = "NRIB".index(p["orientation"])
            pen = 0
            points = set()
            for char in p["text"]:
                row = rows[(p["width"] or p["height"], p["height"], ord(char), turns)]
                dx, dy = ((pen, 0), (0, pen), (-pen, 0), (0, -pen))[turns]
                points.update(
                    (tx + ax + x + dx, ty + ay + y + dy)
                    for x, y in pipeline.pixels(row)
                )
                pen += row["layout_advance"]
            require(
                all(tx <= x < tx + tw and ty <= y < ty + th for x, y in points),
                "candidate escapes native field",
            )
            expected = {
                (x, y) for x, y in reference if tx <= x < tx + tw and ty <= y < ty + th
            }
            require(not (accounted & expected), "reference fields overlap")
            require(not (candidate & points), "candidate fields overlap")
            accounted.update(expected)
            candidate.update(points)
            cases.append(
                dict(
                    text=p["text"],
                    character=p.get("character", p["text"]),
                    width=p["width"],
                    height=p["height"],
                    orientation=p["orientation"],
                    **pipeline.counts(expected, points)
                )
            )
        require(accounted == reference, "unaccounted reference ink")
        stats = pipeline.counts(reference, candidate)
        require(
            all(
                sum(c[k] for c in cases) == stats[k]
                for k in ("under", "over", "xor", "union")
            ),
            "field counts do not cover native canvas",
        )
        results.append(
            dict(name=page["name"], canvas=page["canvas"], cases=cases, **stats)
        )
    return results


def evaluate(plan_path, fit, sampling, output, engine, cache):
    plan = json.loads(plan_path.read_text())
    require(
        plan["schema"] in ("ascii-font-audit-v1", "ascii-spacing-audit-v1"),
        "unknown audit plan",
    )
    if plan["schema"] == "ascii-font-audit-v1":
        require(
            sha((fit / "plan.json").read_bytes()) == plan["fit_plan_sha256"]
            and sha((fit / "report.json").read_bytes()) == plan["fit_report_sha256"]
            and sha((sampling / "plan.json").read_bytes()) == plan["sampling_sha256"],
            "audit inputs changed",
        )
    else:
        require(
            sha((fit / "candidates.json").read_bytes()) == plan["candidates_sha256"]
            and sha((fit / "fit.json").read_bytes()) == plan["fit_sha256"],
            "spacing fit changed",
        )
    require(sha(engine.read_bytes()) == plan["engine_sha256"], "audit engine changed")
    campaigns = []
    queries = set()
    observations = {}
    provenance = []
    for c in plan["captures"]:
        root = sampling / c["name"]
        require(
            sha((root / "manifest.json").read_bytes()) == c["manifest_sha256"],
            "audit request changed",
        )
        capture = json.loads((root / "zd621/capture.json").read_text())
        require(
            datetime.fromisoformat(capture["started_utc"])
            >= datetime.fromisoformat(plan["frozen_at"]),
            "capture predates candidate freeze",
        )
        pages, source = pipeline.load_pages(
            root, "validation", isolated=c["role"] != "spacing"
        )
        require(source["printer"] == plan["printer"], "audit printer differs")
        if c["role"] == "spacing":
            measured = extract(pages)
            require(
                not (observations.keys() & measured.keys()),
                "duplicate spacing observation",
            )
            observations.update(measured)
        campaigns.append((c, pages))
        provenance.append(dict(name=c["name"], **source))
        queries.update(
            (
                p["width"] or p["height"],
                p["height"],
                ord(char),
                "NRIB".index(p["orientation"]),
            )
            for page, _ in pages
            for p in page["probes"]
            for char in p["text"]
        )
    runner = pipeline.Engine(engine, cache, 4)
    report = dict(
        schema=plan["schema"],
        audit_plan_sha256=sha(plan_path.read_bytes()),
        provenance=provenance,
        variants={},
        production_ready=False,
    )
    for font in plan["fonts"]:
        data = (fit / font["file"]).read_bytes()
        require(sha(data) == font["sha256"], "frozen font changed")
        rows = runner.render(data, sorted(queries))
        glyphs = []
        spacing = []
        scored = []
        for c, pages in campaigns:
            scores = text_pages(pages, rows)
            cases = [
                case
                for page in scores
                for case in page["cases"]
                if case["character"] is not None
            ]
            (spacing if c["role"] == "spacing" else glyphs).extend(cases)
            scored.extend(dict(capture=c["name"], role=c["role"], **p) for p in scores)
        gm = metrics(dict(enumerate(glyphs))) if glyphs else None
        sm = metrics(dict(enumerate(spacing)))
        advances = [
            dict(
                character=char,
                size=size,
                measured=value,
                predicted=rows[(size, size, ord(char), 0)]["layout_advance"],
            )
            for (char, size), value in sorted(observations.items())
        ]
        exact = sum(a["measured"] == a["predicted"] for a in advances)
        report["variants"][font["label"]] = dict(
            font_sha256=font["sha256"],
            glyphs=gm,
            spacing_fields=sm,
            advances=dict(cases=len(advances), exact=exact, observations=advances),
            passes_target=(gm is None or gm["passes_target"])
            and sm["passes_target"]
            and exact == len(advances),
            per_glyph={
                c: metrics(dict(enumerate(s for s in glyphs if s["text"] == c)))
                for c in sorted({s["text"] for s in glyphs})
            },
            pages=scored,
        )
        save(output, report)
        print(
            json.dumps(
                dict(variant=font["label"], glyphs=gm, spacing=sm, exact_advances=exact)
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "evaluate"))
    for name in ("fit", "sampling", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--engine", type=Path)
    parser.add_argument("--cache", type=Path)
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(args.fit, args.sampling, args.output)
    else:
        require(
            all((args.plan, args.engine, args.cache)),
            "evaluation requires plan, engine and cache",
        )
        evaluate(
            args.plan, args.fit, args.sampling, args.output, args.engine, args.cache
        )
