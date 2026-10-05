"""Prepare frozen experimental font previews and compare native printer outputs.

Preparation uses only planned Font 0 request metadata. Capture is separately
performed by capture_font_probe.py, with RAM ownership, state witnesses, repeated
controls and cleanup. Neither preparation nor comparison changes a fitted font.
"""

import argparse
import json
from pathlib import Path
import re

from analyze_font_probe import ink
from examine_font_accuracy import breakdown, report_cases
import joint_hint_program as hints
from font_probe import witness_codepoints
from capture_font_probe import MAX_FONT_BYTES
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha


def prepare(fit, variant, fixtures, captures, output, object_name):
    report = json.loads((fit / "report.json").read_text())
    require(
        report["schema"] in ("structured-font-fit-v1", "font-repair-search-v1"),
        "unsupported experimental fit",
    )
    data = (fit / "candidates.json").read_bytes()
    require(
        sha(data) == report["candidates_sha256"]
        and sha((fit / "plan.json").read_bytes()) == report["plan_sha256"],
        "experimental model changed",
    )
    state = json.loads(data)[variant]
    require(
        sha(hints.build(state)) == report["variants"][variant]["font_sha256"],
        "experimental font changed",
    )
    require(
        re.fullmatch(r"R:ZP[0-9A-Z]{1,6}\.TTF", object_name), "unexpected RAM object"
    )
    font = hints.build(state, witnesses=True)
    require(len(font) <= MAX_FONT_BYTES, "constructed font exceeds upload budget")
    pages = [
        dict(
            name="00-state",
            canvas=[384, 64],
            group="state",
            probes=[
                dict(
                    name=name,
                    text=chr(witness_codepoints(state["shapes"])[i]),
                    tile=[i * 64, 0, 64, 64],
                    anchor=[16, 32],
                    width=16,
                    height=16,
                    orientation="N",
                    origin="FT",
                )
                for i, name in enumerate(("identity", "instruction-witness"))
            ],
        )
    ]
    sources = []
    plan = json.loads((fit / "plan.json").read_text())
    fitted = set(map(tuple, plan["search_queries"] + plan["internal_check_queries"]))
    for index, root in enumerate(captures):
        manifest_data = (root / "manifest.json").read_bytes()
        manifest = json.loads(manifest_data)
        require(manifest["schema"] == "small-font-probe-v1", "unexpected source schema")
        sources.append(
            dict(
                path=str(root.resolve().relative_to(fixtures.resolve())),
                manifest_sha256=sha(manifest_data),
            )
        )
        for page in manifest["pages"]:
            require(
                page["group"] == "validation" and page["font"] == "0",
                "replay requires resident validation targets",
            )
            for p in page["probes"]:
                require(p["text"] in state["shapes"], "uncovered glyph")
                q = (
                    p["width"] or p["height"],
                    p["height"],
                    ord(p["text"]),
                    "NRIB".index(p["orientation"]),
                )
                require(q not in fitted, "replay target overlaps fitting")
            pages.append(dict(page, name=f"set{index}-{page['name']}"))
    require(len(pages) + 4 <= 32, "preview budget exceeded")
    output.mkdir(parents=True, exist_ok=False)
    (output / "probe.ttf").write_bytes(font)
    for page in pages:
        w, h = page["canvas"]
        zpl = f"^XA^PW{w}^LL{h}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for p in page["probes"]:
            tx, ty, _, _ = p["tile"]
            ax, ay = p["anchor"]
            text = "".join(f"_{b:02X}" for b in p["text"].encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@{p['orientation']},{p['height']},{p['width']},{object_name}^FH_^FD{text}^FS"
        request = (zpl + "^XZ").encode()
        (output / (page["name"] + ".zpl")).write_bytes(request)
        page["zpl_sha256"] = sha(request)
    save(
        output / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            variant=variant,
            object=object_name,
            font_sha256=sha(font),
            candidates_sha256=report["candidates_sha256"],
            sources=sources,
            pages=pages,
        ),
    )


def compare(root, fixtures, output):
    manifest_data = (root / "manifest.json").read_bytes()
    manifest = json.loads(manifest_data)
    capture = json.loads((root / "zd621/capture.json").read_text())
    require(
        capture["status"] == "complete"
        and capture["cleanup"]["confirmed_absent"]
        and capture["resident_repeat_exact"]
        and capture["font_selection_and_execution_verified"],
        "replay lacks verified controls or cleanup",
    )
    require(
        capture["manifest_sha256"] == sha(manifest_data)
        and manifest["font_sha256"] == sha((root / "probe.ttf").read_bytes()),
        "replay inputs changed",
    )
    report = dict(
        printer=capture["printer"],
        variant=manifest["variant"],
        font_sha256=manifest["font_sha256"],
        candidates_sha256=manifest["candidates_sha256"],
        pages=[],
    )
    for index, source in enumerate(manifest["sources"]):
        source_root = (fixtures / source["path"]).resolve()
        require(
            source_root.is_relative_to(fixtures.resolve()), "source escapes fixtures"
        )
        require(
            sha((source_root / "manifest.json").read_bytes())
            == source["manifest_sha256"],
            "resident manifest changed",
        )
        pages, provenance = pipeline.load_pages(source_root, "validation")
        require(
            provenance["printer"] == capture["printer"], "printer environments differ"
        )
        for page, reference in pages:
            name = f"set{index}-{page['name']}"
            replay = next(p for p in manifest["pages"] if p["name"] == name)
            require(
                replay["probes"] == page["probes"]
                and replay["canvas"] == page["canvas"],
                "canvases or origins differ",
            )
            record = next(p for p in capture["pages"] if p["name"] == name)
            path = root / "zd621" / (name + ".png")
            require(
                sha(path.read_bytes()) == record["png_sha256"], "replay PNG changed"
            )
            require(
                sha((root / (name + ".zpl")).read_bytes())
                == replay["zpl_sha256"]
                == record["zpl_sha256"]
                == sha((root / "zd621" / (name + ".zpl")).read_bytes()),
                "replay request changed",
            )
            candidate = ink(path, page["canvas"])
            cases = []
            for p in page["probes"]:
                tx, ty, tw, th = p["tile"]

                def inside(ps):
                    return {
                        (x, y) for x, y in ps if tx <= x < tx + tw and ty <= y < ty + th
                    }

                cases.append(
                    dict(
                        text=p["text"],
                        width=p["width"],
                        height=p["height"],
                        orientation=p["orientation"],
                        **pipeline.counts(inside(reference), inside(candidate)),
                    )
                )
            report["pages"].append(
                dict(
                    name=name,
                    canvas=page["canvas"],
                    reference_png_sha256=next(
                        p["png_sha256"]
                        for p in provenance["pages"]
                        if p["name"] == page["name"]
                    ),
                    candidate_png_sha256=record["png_sha256"],
                    cases=cases,
                    **pipeline.counts(reference, candidate),
                )
            )
    report["breakdown"] = breakdown(report_cases(report["pages"]))
    save(output, report)
    print(json.dumps(report["breakdown"]["sizes"], indent=2))


def paired(baseline_path, candidate_path, output):
    before, after = [json.loads(p.read_text()) for p in (baseline_path, candidate_path)]
    require(before["printer"] == after["printer"], "paired printer environments differ")
    pages = []
    for a, b in zip(before["pages"], after["pages"], strict=True):
        require(
            all(a[k] == b[k] for k in ("name", "canvas", "reference_png_sha256")),
            "paired resident targets differ",
        )
        old, new = pipeline.totals([a]), pipeline.totals([b])
        lost = []
        for x, y in zip(a["cases"], b["cases"], strict=True):
            require(
                all(x[k] == y[k] for k in ("text", "width", "height", "orientation")),
                "paired case origins differ",
            )
            if x["xor"] == 0 and y["xor"]:
                lost.append(
                    {k: x[k] for k in ("text", "width", "height", "orientation")}
                )
        pages.append(
            dict(
                name=a["name"],
                baseline=old,
                candidate=new,
                lost_exact=lost,
                passes=old["iou"] <= new["iou"] and old["exact"] <= new["exact"],
            )
        )
    result = dict(
        printer=before["printer"],
        baseline_report_sha256=sha(baseline_path.read_bytes()),
        candidate_report_sha256=sha(candidate_path.read_bytes()),
        pages=pages,
        passes_baseline_page_gate=all(p["passes"] for p in pages),
        baseline=breakdown(report_cases(before["pages"])),
        candidate=breakdown(report_cases(after["pages"])),
    )
    save(output, result)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    p = commands.add_parser("prepare")
    for name in ("fit", "fixtures", "output"):
        p.add_argument(name, type=Path)
    p.add_argument("--variant", required=True)
    p.add_argument("--object", required=True)
    p.add_argument("--captures", nargs="+", type=Path, required=True)
    p = commands.add_parser("compare")
    for name in ("root", "fixtures", "output"):
        p.add_argument(name, type=Path)
    p = commands.add_parser("paired")
    for name in ("baseline", "candidate", "output"):
        p.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(
            args.fit,
            args.variant,
            args.fixtures,
            args.captures,
            args.output,
            args.object,
        )
    elif args.action == "compare":
        compare(args.root, args.fixtures, args.output)
    else:
        paired(args.baseline, args.candidate, args.output)
