"""Evaluate the frozen six-glyph model on complete native printer canvases.

The first validation set informed the choice of relationships; only the second
reserved campaign is independent of that refinement. Production bitmap rendering
is reported separately from the unhinted outline initializer.
"""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from capture_font_probe import sha
from compare_constructed import compare as replay
import font0_hint_model as hints


def summarize(pages):
    result = {
        key: sum(p[key] for p in pages) for key in ["under", "over", "xor", "union"]
    }
    result["iou"] = 1 - result["xor"] / result["union"]
    cases = [c for p in pages for c in p["cases"]]
    result["exact"] = sum(c["xor"] == 0 for c in cases)
    result["cases"] = len(cases)
    return result


def production(pages, root, renderer):
    result = []
    with tempfile.TemporaryDirectory() as directory:
        for page, reference in pages:
            output = Path(directory) / (page["name"] + ".png")
            subprocess.run(
                [
                    str(renderer.resolve()),
                    "render",
                    str(root / (page["name"] + ".zpl")),
                    str(output),
                    "--profile",
                    "zd621",
                ],
                check=True,
                capture_output=True,
                timeout=120,
            )
            candidate = hints.ink(output, page["canvas"])
            cases = []
            for p in page["probes"]:
                tx, ty, tw, th = p["tile"]
                expected = {
                    (x, y)
                    for x, y in reference
                    if tx <= x < tx + tw and ty <= y < ty + th
                }
                actual = {
                    (x, y)
                    for x, y in candidate
                    if tx <= x < tx + tw and ty <= y < ty + th
                }
                cases.append(
                    dict(
                        text=p["text"],
                        width=p["width"],
                        height=p["height"],
                        orientation=p["orientation"],
                        **hints.counts(expected, actual)
                    )
                )
            stats = hints.counts(reference, candidate)
            result.append(
                dict(
                    name=page["name"],
                    **stats,
                    iou=1 - stats["xor"] / stats["union"],
                    cases=cases
                )
            )
    return result


def evaluate(source, development, validation, generated, engine, renderer, swiss):
    model = json.loads((generated / "model.json").read_text())
    assert model["source_manifest_sha256"] == sha(
        (source / "manifest.json").read_bytes()
    )
    assert model["development_manifest_sha256"] == sha(
        (development / "manifest.json").read_bytes()
    )
    shapes = hints.load_models(source)
    hinted = hints.compile_hints(
        shapes, model["hints"], model["parameters"], witnesses=True
    )
    assert hinted == (generated / "probe.ttf").read_bytes()
    plain = hints.build_font(shapes)
    report = dict(
        schema="small-font-hints-evaluation-v1",
        model_sha256=sha((generated / "model.json").read_bytes()),
        font_sha256=sha(hinted),
        sets={},
        totals={},
    )
    for label, root, group in [
        ("development", development, "development"),
        ("reused-validation", development, "validation"),
        ("reserved", validation, "validation"),
    ]:
        pages = hints.load_pages(root, "0", group)
        queries = hints.queries_for(pages)
        report["sets"][label] = {}
        for name, font in [("plain", plain), ("hinted", hinted)]:
            scored = hints.score(pages, hints.engine_rows(engine, font, queries))
            report["sets"][label][name] = scored
            report["totals"][label + "/" + name] = summarize(scored)
        scored = production(pages, root, renderer)
        report["sets"][label]["production"] = scored
        report["totals"][label + "/production"] = summarize(scored)
    if swiss is not None:
        from swiss_probe import FONT_SHA

        assert sha(swiss.read_bytes()) == FONT_SHA
        report["swiss_font_sha256"] = FONT_SHA
        for group in ["development", "validation"]:
            pages = hints.load_pages(development, "swiss", group)
            scored = hints.score(
                pages,
                hints.engine_rows(engine, swiss.read_bytes(), hints.queries_for(pages)),
            )
            report["sets"]["swiss-" + group] = scored
            report["totals"]["swiss-" + group] = summarize(scored)
    report["generated_font_replay"] = replay(generated, engine)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["source", "development", "validation", "generated"]:
        parser.add_argument(name, type=Path)
    for name in ["engine", "renderer", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--swiss", type=Path)
    args = parser.parse_args()
    report = evaluate(
        args.source,
        args.development,
        args.validation,
        args.generated,
        args.engine,
        args.renderer,
        args.swiss,
    )
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["totals"], indent=2))
