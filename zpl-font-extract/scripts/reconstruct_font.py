"""Fit, freeze and independently evaluate an original reconstructed TrueType font.

Fit reads only development PNGs. Evaluate cannot alter the frozen model. Native
canvases and declared FT origins are preserved throughout; no image registration
or per-size correction tables are used. See docs/font-reconstruction.md.
"""

import argparse
import itertools
import json
import math
from pathlib import Path
import re

from analyze_font_probe import ink
from capture_font_probe import sha
from compare_swiss import counts, pixels
import font0_hint_model as engine_api
import reconstruct_geometry as geometry
import reconstruct_hints as hints

SCHEMA = "automatic-font-reconstruction-v1"


def save(path, value):
    data = json.dumps(value, indent=2, sort_keys=True) + "\n"
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(data)
    temporary.replace(path)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_pages(root, group):
    """Read selected pixels only; validation-page contents are never fit inputs."""
    manifest_data = (root / "manifest.json").read_bytes()
    manifest = json.loads(manifest_data)
    capture = json.loads((root / "zd621/capture.json").read_text())
    observation = root / "observation.json"
    require(
        not observation.exists()
        or json.loads(observation.read_text()).get("status") != "diagnostic-only",
        "diagnostic-only capture is not reconstruction evidence",
    )
    require(
        manifest["schema"] in ("font0-outline-pilot-v1", "small-font-probe-v1"),
        "unsupported capture schema",
    )
    require(
        capture["status"] == "complete"
        and capture["resident_repeat_exact"]
        and capture["outline_repeat_exact"],
        "capture lacks repeated controls",
    )
    require(
        capture["manifest_sha256"] == sha(manifest_data), "capture manifest changed"
    )
    printer = capture["printer"]
    require(
        (printer["model"], printer["dpi"], printer["firmware"])
        == ("ZD621", 203, "V93.21.33Z"),
        "capture does not match the calibrated rendering environment",
    )
    records = {p["name"]: p for p in capture["pages"]}
    for name in ("resident-start", "resident-end", "outline-repeat"):
        data = (root / "zd621" / (name + ".png")).read_bytes()
        require(sha(data) == records[name]["png_sha256"], "control PNG changed")
    require(
        records["resident-start"]["png_sha256"]
        == records["resident-end"]["png_sha256"],
        "resident control changed",
    )
    require(
        records["outline-repeat"]["png_sha256"]
        == records[manifest["pages"][0]["name"]]["png_sha256"],
        "outline control changed",
    )
    pages = []
    for page in manifest["pages"]:
        if page["group"] != group or page.get("font", "0") != "0":
            continue
        name = page["name"]
        require(Path(name).name == name, "invalid page name")
        record = records[name]
        require(record["status"] == "captured", "incomplete page")
        for base in (root, root / "zd621"):
            require(
                sha((base / (name + ".zpl")).read_bytes())
                == page["zpl_sha256"]
                == record["zpl_sha256"],
                "page request changed",
            )
        path = root / "zd621" / (name + ".png")
        require(sha(path.read_bytes()) == record["png_sha256"], "page PNG changed")
        require(
            page["canvas"] == record["dimensions"]
            and 0 < page["canvas"][0] * page["canvas"][1] <= 4_000_000,
            "invalid native canvas",
        )
        page = json.loads(json.dumps(page))
        for p in page["probes"]:
            require(
                len(p["text"]) == 1
                and 32 <= ord(p["text"]) <= 126
                and p["origin"] == "FT",
                "requires isolated ASCII FT probes",
            )
            if manifest["schema"] == "font0-outline-pilot-v1":
                p["anchor"] = [p["anchor"][a] - p["tile"][a] for a in (0, 1)]
            require(
                p["orientation"] in "NRIB" and len(p["orientation"]) == 1,
                "invalid rotation",
            )
            require(
                0 < p["height"] <= 1024 and 0 <= p["width"] <= 1024,
                "size outside reconstruction budget",
            )
        pages.append((page, ink(path, page["canvas"])))
    require(0 < len(pages) <= 128, "empty or excessive page selection")
    return pages, dict(
        manifest_sha256=sha(manifest_data),
        printer=printer,
        pages=[
            dict(name=p["name"], png_sha256=records[p["name"]]["png_sha256"])
            for p, _ in pages
        ],
    )


def cases(pages):
    result = {}
    for page, reference in pages:
        for p in page["probes"]:
            tx, ty, tw, th = p["tile"]
            ax, ay = p["anchor"]
            key = (
                p["width"] or p["height"],
                p["height"],
                ord(p["text"]),
                "NRIB".index(p["orientation"]),
            )
            selected = {
                (x - tx - ax, y - ty - ay)
                for x, y in reference
                if tx <= x < tx + tw and ty <= y < ty + th
            }
            require(key not in result, "duplicate glyph/size/orientation case")
            result[key] = selected
    return result


def loss(reference, rendered, keys):
    errors = []
    for key in keys:
        stats = counts(reference[key], pixels(rendered[key]))
        errors.append(stats["xor"] / stats["union"] if stats["union"] else 0)
    require(bool(errors), "empty scoring partition")
    return sum(errors) / len(errors)


class Engine:
    """Content-addressed, bounded render cache. Resume replays identical decisions."""

    def __init__(self, binary, directory, budget):
        self.binary = binary.resolve()
        self.digest = sha(self.binary.read_bytes())
        self.directory = directory
        directory.mkdir(exist_ok=True)
        self.budget = budget
        self.seen = set()
        self.executed = 0

    def render(self, data, keys):
        keys = sorted(keys)
        digest = sha(data + json.dumps(keys).encode() + self.digest.encode())
        self.seen.add(digest)
        require(
            len(self.seen) <= self.budget,
            "render budget exhausted; increase --max-evaluations and resume",
        )
        path = self.directory / (digest + ".json")
        if path.exists():
            record = json.loads(path.read_text())
            require(
                record["key"] == digest
                and record["rows_sha256"]
                == sha(json.dumps(record["rows"], sort_keys=True).encode()),
                "render cache changed",
            )
            require(
                len(record["rows"]) == len(keys), "render cache query count changed"
            )
            return dict(zip(keys, record["rows"], strict=True))
        rows = engine_api.engine_rows(self.binary, data, keys)
        values = [{k: v for k, v in rows[q].items() if k != "contours"} for q in keys]
        save(
            path,
            dict(
                key=digest,
                rows=values,
                rows_sha256=sha(json.dumps(values, sort_keys=True).encode()),
            ),
        )
        self.executed += 1
        return rows


def source_shapes(pages):
    references = cases(pages)
    selected = {}
    for key, points in references.items():
        x, y, code, turn = key
        if (
            turn == 0
            and x == y
            and points
            and (code not in selected or y > selected[code][0][1])
        ):
            selected[code] = (key, points)
    require(bool(selected), "no nonblank upright source silhouettes")
    for page, _ in pages:
        for p in page["probes"]:
            entry = selected.get(ord(p["text"]))
            if entry and p["height"] == entry[0][1] and p["orientation"] == "N":
                ax, ay = p["anchor"]
                _, _, width, height = p["tile"]
                require(
                    all(
                        1 < x + ax < width - 2 and 1 < y + ay < height - 2
                        for x, y in entry[1]
                    ),
                    "source silhouette touches tile boundary",
                )
    return {chr(code): pair for code, pair in sorted(selected.items())}


def ppem(dots):
    # Measured ZD621 V93 point quantization; zpl::truetype::Environment::ppem.
    return (max(dots, 10) * 1152 // 203) * 184958 / (16 * 65536)


def fit(source, development, output, binary, resume=False, budget=600):
    if resume:
        require(
            output.is_dir()
            and json.loads((output / "progress.json").read_text())["status"]
            != "complete",
            "resume requires an incomplete run",
        )
    else:
        output.mkdir(parents=True, exist_ok=False)
    progress = dict(schema=SCHEMA, status="running", stages=[])
    try:
        large_pages, source_provenance = load_pages(source, "development")
        small_pages, development_provenance = load_pages(development, "development")
        require(
            source_provenance["printer"] == development_provenance["printer"],
            "capture environments differ",
        )
        runner = Engine(binary, output / "cache", budget)
        scripts = [
            Path(__file__),
            Path(geometry.__file__),
            Path(hints.__file__),
            Path(engine_api.__file__),
            Path(hints.font_probe.__file__),
        ]
        scripts += [
            Path(__file__).with_name(name + ".py")
            for name in (
                "analyze_font0_outline",
                "analyze_font_probe",
                "compare_swiss",
                "capture_font_probe",
            )
        ]
        seal = dict(
            schema=SCHEMA,
            source=source_provenance,
            development=development_provenance,
            engine_sha256=runner.digest,
            scripts={p.name: sha(p.read_bytes()) for p in scripts},
            tolerances=[0.65, 0.9, 1.25, 1.75],
            complexity_penalty=1e-5,
            hint_cutin=24,
            selection="development-only; every third size configuration reserved for hint acceptance",
        )
        seal_path = output / "inputs.json"
        if resume:
            require(
                json.loads(seal_path.read_text()) == seal,
                "resume inputs, scripts, or rendering engine changed",
            )
        else:
            save(seal_path, seal)
        save(output / "progress.json", progress)
        large = cases(large_pages)
        small = cases(small_pages)
        sources = source_shapes(large_pages)
        require(
            set(map(ord, sources)) == {k[2] for k in small},
            "source and development glyph sets differ",
        )
        shapes, initial, decisions = {}, {}, {}
        for char, (source_key, points) in sources.items():
            print(f"Geometry {char!r}", flush=True)
            # Preserve the previous polygon initializer as the acceptance floor.
            scale = 2048 / source_key[1]
            polygon = [
                [(round(x * scale), round(-y * scale), True) for x, y in contour]
                for contour in engine_api.contours(points)
            ]
            initial[char] = polygon
            keys = [q for q in large if q[2] == ord(char)]
            choices = [("polygon", polygon)]
            rejected = []
            for tolerance in seal["tolerances"]:
                try:
                    shape = geometry.fit_shape(
                        points, tolerance, 2048 / ppem(source_key[1])
                    )
                    choices.append((f"quadratic-{tolerance}", shape))
                except geometry.FitRejected as error:
                    rejected.append(
                        dict(candidate=f"quadratic-{tolerance}", reason=str(error))
                    )
            scores = []
            for label, shape in choices:
                rows = runner.render(engine_api.build_font({char: shape}), keys)
                scores.append(
                    dict(
                        candidate=label,
                        loss=loss(large, rows, keys),
                        points=sum(map(len, shape)),
                    )
                )
            winner = min(
                range(len(choices)),
                key=lambda i: (
                    scores[i]["loss"]
                    + scores[i]["points"] * seal["complexity_penalty"],
                    scores[i]["points"],
                ),
            )
            # Compactness never justifies worse measured geometry.
            if scores[winner]["loss"] > scores[0]["loss"]:
                winner = 0
            shapes[char] = choices[winner][1]
            decisions[char] = dict(
                selected=choices[winner][0], candidates=scores, rejected=rejected
            )
            progress["stages"].append(
                dict(glyph=char, stage="geometry", **decisions[char])
            )
            save(output / "progress.json", progress)
        inferred = hints.infer(shapes)
        max_small = max(max(q[:2]) for q in small)
        min_large = min(min(q[:2]) for q in large)
        require(min_large > max_small, "geometry and hint size bands must be separate")
        inferred["hint_ppem_limit"] = math.floor(math.sqrt(max_small * min_large))
        configurations = sorted({(q[0], q[1], q[3]) for q in small})
        acceptance = set(configurations[1::3])
        require(
            len(acceptance) >= 2 and len(configurations) - len(acceptance) >= 3,
            "insufficient independent development sizes",
        )
        policies, hint_decisions = {}, {}
        for char in shapes:
            print(f"Hints {char!r}", flush=True)
            training = [
                q
                for q in small
                if q[2] == ord(char) and (q[0], q[1], q[3]) not in acceptance
            ]
            validation = [
                q
                for q in small
                if q[2] == ord(char) and (q[0], q[1], q[3]) in acceptance
            ]
            keys = training + validation
            geometry_keys = [q for q in large if q[2] == ord(char)]
            geometry_baseline = runner.render(
                engine_api.build_font({char: shapes[char]}), geometry_keys
            )
            choices = list(itertools.product(hints.POLICIES, repeat=2))
            scored = []
            for policy in choices:
                rows = runner.render(
                    hints.build({char: shapes[char]}, inferred, {char: policy}),
                    keys + geometry_keys,
                )
                require(
                    all(
                        pixels(rows[q]) == pixels(geometry_baseline[q])
                        for q in geometry_keys
                    ),
                    "hint program changed previously accepted large-size geometry",
                )
                scored.append(
                    dict(
                        policy=policy,
                        training_loss=loss(small, rows, training),
                        acceptance_loss=loss(small, rows, validation),
                    )
                )
            winner = min(
                range(len(scored)),
                key=lambda i: (
                    scored[i]["training_loss"],
                    sum(p != "none" for p in choices[i]),
                    i,
                ),
            )
            proposed = winner
            accepted = scored[winner]["acceptance_loss"] <= scored[0]["acceptance_loss"]
            if not accepted:
                winner = 0
            policies[char] = choices[winner]
            hint_decisions[char] = dict(
                selected=choices[winner],
                proposed=choices[proposed],
                accepted=accepted,
                training_queries=training,
                acceptance_queries=validation,
                candidates=scored,
            )
            progress["stages"].append(
                dict(
                    glyph=char,
                    stage="hints",
                    selected=choices[winner],
                    accepted=accepted,
                )
            )
            save(output / "progress.json", progress)
        model = dict(
            schema=SCHEMA,
            inputs_sha256=sha(seal_path.read_bytes()),
            shapes=shapes,
            inferred=inferred,
            policies=policies,
            cutin=24,
            geometry_decisions=decisions,
            hint_decisions=hint_decisions,
            spacing="placeholder advances; isolated-glyph reconstruction only",
            training_queries=sorted(set(large) | set(small)),
        )
        data = hints.build(shapes, inferred, policies)
        (output / "font.ttf").write_bytes(data)
        (output / "geometry.ttf").write_bytes(engine_api.build_font(shapes))
        (output / "initial.ttf").write_bytes(engine_api.build_font(initial))
        model["artifacts"] = {
            name: sha((output / name).read_bytes())
            for name in ("font.ttf", "geometry.ttf", "initial.ttf")
        }
        save(output / "model.json", model)
        progress.update(
            status="complete",
            unique_evaluations=len(runner.seen),
            executed_evaluations=runner.executed,
            model_sha256=sha((output / "model.json").read_bytes()),
        )
        save(output / "progress.json", progress)
        print(
            json.dumps(
                dict(status="complete", evaluations=len(runner.seen), policies=policies)
            ),
            flush=True,
        )
    except BaseException as error:
        progress.update(status="failed", error=str(error))
        save(output / "progress.json", progress)
        raise


def frozen(root):
    model_data = (root / "model.json").read_bytes()
    model = json.loads(model_data)
    if model["schema"] == "joint-font-optimization-v1":
        from optimize_font import frozen as joint_frozen

        return joint_frozen(root)
    require(model["schema"] == SCHEMA, "unknown reconstruction model")
    require(
        set(model["artifacts"]) == {"font.ttf", "geometry.ttf", "initial.ttf"},
        "incomplete frozen artifacts",
    )
    require(
        sha((root / "inputs.json").read_bytes()) == model["inputs_sha256"],
        "reconstruction inputs changed",
    )
    progress = json.loads((root / "progress.json").read_text())
    require(
        progress["status"] == "complete"
        and progress["model_sha256"] == sha(model_data),
        "model is not frozen",
    )
    for name, digest in model["artifacts"].items():
        require(
            name in ("font.ttf", "geometry.ttf", "initial.ttf"),
            "unknown model artifact",
        )
        require(sha((root / name).read_bytes()) == digest, "frozen font changed")
    require(
        hints.build(
            model["shapes"], model["inferred"], model["policies"], model["cutin"]
        )
        == (root / "font.ttf").read_bytes(),
        "font no longer reproduces from frozen model",
    )
    return model, sha(model_data)


def compile_model(model, witnesses=False):
    if model["schema"] == "joint-font-optimization-v1":
        from joint_hint_program import build

        return build(model["state"], witnesses=witnesses)
    return hints.build(
        model["shapes"],
        model["inferred"],
        model["policies"],
        model["cutin"],
        witnesses=witnesses,
    )


def totals(scored):
    result = {k: sum(p[k] for p in scored) for k in ("under", "over", "xor", "union")}
    result["iou"] = 1 - result["xor"] / result["union"] if result["union"] else 1
    result["exact"] = sum(c["xor"] == 0 for p in scored for c in p["cases"])
    result["cases"] = sum(len(p["cases"]) for p in scored)
    return result


def evaluate(root, captures, binary, output, group="validation", renderer=None):
    model, digest = frozen(root)
    seal = json.loads((root / "inputs.json").read_text())
    require(
        sha(binary.read_bytes()) == seal["engine_sha256"],
        "evaluation engine differs from fitting engine",
    )
    report = dict(
        schema=model["schema"], model_sha256=digest, group=group, campaigns=[]
    )
    require(
        output.resolve()
        not in {
            (root / p).resolve()
            for p in ("model.json", "inputs.json", "progress.json", *model["artifacts"])
        },
        "evaluation cannot overwrite frozen artifacts",
    )
    runner = Engine(binary, root / "cache", 32)
    for directory in captures:
        pages, provenance = load_pages(directory, group)
        require(
            provenance["printer"] == seal["source"]["printer"],
            "evaluation capture environment differs",
        )
        queries = sorted(cases(pages))
        overlap = set(queries) & {tuple(q) for q in model["training_queries"]}
        if group == "validation":
            require(not overlap, "validation overlaps fitting queries")
        require(
            {chr(q[2]) for q in queries} <= set(model["shapes"]),
            "validation contains uncovered glyphs",
        )
        result = dict(
            capture=directory.name, provenance=provenance, fonts={}, totals={}
        )
        fonts = ["initial.ttf", "geometry.ttf", "font.ttf"]
        if "proposal.ttf" in model["artifacts"]:
            fonts.append("proposal.ttf")
        for name in fonts:
            rows = runner.render((root / name).read_bytes(), queries)
            scored = engine_api.score(pages, rows)
            result["fonts"][name] = scored
            result["totals"][name] = totals(scored)
        if renderer is not None:
            from evaluate_font0_hints import production

            scored = production(pages, directory, renderer)
            result["fonts"]["production"] = scored
            result["totals"]["production"] = totals(scored)

        # Evaluation reports accept or reject; they never select another model.
        def nonregression(name, candidate="font.ttf"):
            return all(
                totals([a])["iou"] >= totals([b])["iou"]
                and totals([a])["exact"] >= totals([b])["exact"]
                for a, b in zip(
                    result["fonts"][candidate], result["fonts"][name], strict=True
                )
            )

        result["passes_outline_gate"] = nonregression("initial.ttf")
        result["passes_production_gate"] = (
            nonregression("production") if renderer is not None else None
        )
        if "proposal.ttf" in fonts:
            result["passes_proposal_outline_gate"] = nonregression(
                "initial.ttf", "proposal.ttf"
            )
            result["passes_proposal_production_gate"] = (
                nonregression("production", "proposal.ttf")
                if renderer is not None
                else None
            )
        report["campaigns"].append(result)
    report["passes_outline_gate"] = all(
        p["passes_outline_gate"] for p in report["campaigns"]
    )
    if "proposal.ttf" in model["artifacts"]:
        report["passes_proposal_outline_gate"] = all(
            p["passes_proposal_outline_gate"] for p in report["campaigns"]
        )
    report["production_ready"] = False
    save(output, report)
    print(
        json.dumps(
            [
                dict(
                    capture=p["capture"],
                    totals=p["totals"],
                    passes_outline_gate=p["passes_outline_gate"],
                    **(
                        {
                            "passes_proposal_outline_gate": p[
                                "passes_proposal_outline_gate"
                            ]
                        }
                        if "passes_proposal_outline_gate" in p
                        else {}
                    ),
                )
                for p in report["campaigns"]
            ],
            indent=2,
        )
    )


def prepare_preview(
    root, captures, output, object_name, group="validation", variant="font"
):
    """Prepare requests only; the existing guarded capture tool owns transport."""
    model, digest = frozen(root)
    require(variant in ("font", "proposal"), "unknown preview variant")
    if variant == "proposal":
        require(
            model["schema"] == "joint-font-optimization-v1",
            "model has no joint proposal",
        )
        model = dict(model, state=model["proposal"], shapes=model["proposal"]["shapes"])
    require(
        re.fullmatch(r"R:ZP[0-9A-Z]{1,6}\.TTF", object_name),
        "unexpected RAM object name",
    )
    data = compile_model(model, witnesses=True)
    require(len(data) <= 65536, "constructed font exceeds upload budget")
    state = [
        dict(
            name=name,
            text=chr(33 + i),
            tile=[i * 64, 0, 64, 64],
            anchor=[16, 32],
            width=16,
            height=16,
            orientation="N",
            origin="FT",
        )
        for i, name in enumerate(("identity", "instruction-witness"))
    ]
    pages = [dict(name="00-state", canvas=[384, 64], probes=state, group="state")]
    provenance = []
    for i, directory in enumerate(captures):
        selected, source = load_pages(directory, group)
        require(
            source["printer"]
            == json.loads((root / "inputs.json").read_text())["source"]["printer"],
            "preview source environment differs",
        )
        provenance.append(source)
        for page, _ in selected:
            require(
                all(p["text"] in model["shapes"] for p in page["probes"]),
                "preview contains uncovered glyphs",
            )
            pages.append(dict(page, name=f"set{i}-{page['name']}"))
    require(len(pages) + 4 <= 32, "campaign exceeds preview budget")
    output.mkdir(parents=True, exist_ok=False)
    (output / "probe.ttf").write_bytes(data)
    for page in pages:
        width, height = page["canvas"]
        zpl = f"^XA^PW{width}^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
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
            object=object_name,
            font_sha256=sha(data),
            model_sha256=digest,
            sources=provenance,
            pages=pages,
            **(
                {"variant": variant}
                if model["schema"] == "joint-font-optimization-v1"
                else {}
            ),
        ),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    fit_parser = actions.add_parser(
        "fit", help="development data only; export a frozen research font"
    )
    fit_parser.add_argument("source", type=Path)
    fit_parser.add_argument("development", type=Path)
    fit_parser.add_argument("output", type=Path)
    fit_parser.add_argument("--engine", type=Path, required=True)
    fit_parser.add_argument("--resume", action="store_true")
    fit_parser.add_argument("--max-evaluations", type=int, default=600)
    evaluate_parser = actions.add_parser(
        "evaluate", help="read a frozen model and report native-canvas accuracy"
    )
    evaluate_parser.add_argument("model", type=Path)
    evaluate_parser.add_argument("captures", type=Path, nargs="+")
    evaluate_parser.add_argument("--engine", type=Path, required=True)
    evaluate_parser.add_argument("--output", type=Path, required=True)
    evaluate_parser.add_argument(
        "--group", choices=("development", "validation"), default="validation"
    )
    evaluate_parser.add_argument("--renderer", type=Path)
    preview_parser = actions.add_parser(
        "prepare-preview",
        help="prepare a frozen-font upload campaign; no network access",
    )
    preview_parser.add_argument("model", type=Path)
    preview_parser.add_argument("captures", type=Path, nargs="+")
    preview_parser.add_argument("--output", type=Path, required=True)
    preview_parser.add_argument(
        "--object",
        required=True,
        help="fresh R:ZP...TTF name to avoid stale printer font state",
    )
    preview_parser.add_argument(
        "--group", choices=("development", "validation"), default="validation"
    )
    preview_parser.add_argument(
        "--variant", choices=("font", "proposal"), default="font"
    )
    args = parser.parse_args()
    try:
        if args.action == "fit":
            require(
                1 <= args.max_evaluations <= 10000, "evaluation budget must be 1..10000"
            )
            fit(
                args.source,
                args.development,
                args.output,
                args.engine,
                args.resume,
                args.max_evaluations,
            )
        elif args.action == "evaluate":
            evaluate(
                args.model,
                args.captures,
                args.engine,
                args.output,
                args.group,
                args.renderer,
            )
        else:
            prepare_preview(
                args.model,
                args.captures,
                args.output,
                args.object,
                args.group,
                args.variant,
            )
    except (ValueError, OSError) as error:
        parser.exit(1, f"reconstruct_font: {error}\n")


if __name__ == "__main__":
    main()
