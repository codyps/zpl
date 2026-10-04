"""Bounded mixed geometry/CVT/hint-program optimization of captured glyphs.

The continuous distance-field guide proposes coordinates; exact native-origin
rasters decide every move. A beam searches a typed, acyclic hint language rather
than arbitrary TrueType bytes. Only development captures enter the search.
"""

import argparse
from copy import deepcopy
import json
from pathlib import Path

import joint_font_geometry as geometry
import joint_hint_program as hints
import reconstruct_font as pipeline
from reconstruct_font import require, save, sha

SCHEMA = "joint-font-optimization-v1"
ARTIFACTS = {"initial.ttf", "geometry.ttf", "font.ttf", "proposal.ttf"}


def category(query):
    x, y, _, turn = query
    return (
        "large"
        if min(x, y) >= 128
        else "square" if x == y and turn == 0 else "transformed"
    )


def partition(large, small):
    configurations = sorted({(q[0], q[1], q[3]) for q in small})
    reserved = set(configurations[1::3])
    training = {q: p for q, p in small.items() if (q[0], q[1], q[3]) not in reserved}
    acceptance = {q: p for q, p in small.items() if (q[0], q[1], q[3]) in reserved}
    require(not (set(large) & set(small)), "large and small cases overlap")
    require(
        bool(training) and bool(acceptance), "insufficient development configurations"
    )
    return large | training, acceptance


def means(errors):
    grouped = {}
    for q, error in errors.items():
        grouped.setdefault(category(q), []).append(error)
    return {name: sum(values) / len(values) for name, values in sorted(grouped.items())}


def objective(state, baseline, errors):
    groups = means(errors)
    balanced = sum(groups.values()) / len(groups) + 0.2 * max(groups.values())
    program = sum(hints.complexity(p) for p in state["programs"].values()) / len(
        state["programs"]
    )
    shifts = [
        (p[a] - b[a]) ** 2
        for c, shape in state["shapes"].items()
        for contour, original in zip(shape, baseline["shapes"][c], strict=True)
        for p, b in zip(contour, original, strict=True)
        for a in (0, 1)
    ]
    params, prior = state["parameters"], baseline["parameters"]
    numeric = [
        (a - b) ** 2
        for axis, old in zip(params["widths"], prior["widths"], strict=True)
        for a, b in zip(axis, old, strict=True)
    ]
    numeric += [
        (a - b) ** 2 for a, b in zip(params["zones"], prior["zones"], strict=True)
    ]
    numeric.append((params["cutin"] - prior["cutin"]) ** 2)
    return (
        balanced
        + 1e-5 * program
        + 1e-6 * sum(shifts) / len(shifts)
        + 1e-7 * sum(numeric) / len(numeric)
    )


class Oracle:
    def __init__(self, engine, references, baseline):
        self.engine, self.references, self.baseline = engine, references, baseline
        self.memo = {}
        self.floor = self.errors(baseline)

    def errors(self, state):
        errors = {}
        for char in sorted(state["shapes"]):
            keys = sorted(q for q in self.references if q[2] == ord(char))
            require(bool(keys), "glyph lacks scoring cases")
            data = hints.build(state, [char])
            # The compiler guards every glyph instruction by both axes' MPPEM.
            # Beyond its fixed limit, point coordinates are unaffected by hints
            # or CVT values. Reuse those exact scores across program candidates;
            # still render each distinct outline, origin, size and rotation.
            plain = [
                q
                for q in keys
                if pipeline.ppem(max(q[:2])) > state["parameters"]["limit"] + 1
            ]
            plain_keys = set(plain)
            hinted = [q for q in keys if q not in plain_keys]
            for selected, identity in (
                (
                    plain,
                    json.dumps(
                        [state["shapes"][char], state["parameters"]["limit"]],
                        sort_keys=True,
                    ).encode(),
                ),
                (hinted, data),
            ):
                if not selected:
                    continue
                digest = sha(identity + json.dumps(selected).encode())
                if digest not in self.memo:
                    rows = self.engine.render(data, selected)
                    measured = {}
                    for q in selected:
                        stats = pipeline.counts(
                            self.references[q], pipeline.pixels(rows[q])
                        )
                        measured[q] = (
                            stats["xor"] / stats["union"] if stats["union"] else 0
                        )
                    self.memo[digest] = measured
                errors.update(self.memo[digest])
        return errors

    def score(self, state):
        errors = self.errors(state)
        # Every large development sample is an individual constraint, not a
        # weighted term that a small-size gain can silently overwhelm.
        if any(
            e > self.floor[q] + 1e-12
            for q, e in errors.items()
            if category(q) == "large"
        ):
            return float("inf")
        return objective(state, self.baseline, errors)


def numeric_neighbors(state, baseline, step):
    params, prior = state["parameters"], baseline["parameters"]
    for axis, widths in enumerate(params["widths"]):
        for index, value in enumerate(widths):
            for delta in (-step, step):
                candidate = deepcopy(state)
                new = value + delta
                if new > 0 and abs(new - prior["widths"][axis][index]) <= 32:
                    candidate["parameters"]["widths"][axis][index] = new
                    if candidate["parameters"]["widths"][axis] == sorted(
                        set(candidate["parameters"]["widths"][axis])
                    ):
                        yield candidate, f"width-{axis}-{index}:{delta}"
    for index, value in enumerate(params["zones"]):
        for delta in (-step, step):
            candidate = deepcopy(state)
            new = value + delta
            if abs(new - prior["zones"][index]) <= 16:
                candidate["parameters"]["zones"][index] = new
                if candidate["parameters"]["zones"] == sorted(
                    set(candidate["parameters"]["zones"])
                ):
                    yield candidate, f"zone-{index}:{delta}"
    for delta in (-step, step):
        new = params["cutin"] + delta
        if 0 <= new <= 64 and abs(new - prior["cutin"]) <= 8:
            candidate = deepcopy(state)
            candidate["parameters"]["cutin"] = new
            yield candidate, f"cutin:{delta}"


def beam(state, char, oracle, width, depth, limit, round_index):
    frontier = [(oracle.score(state), state)]
    visited = {json.dumps(state["programs"][char], sort_keys=True)}
    best = frontier[0]
    for level in range(depth):
        pool = list(frontier)
        for _, parent in frontier:
            candidates = {}
            for program in hints.neighbors(parent, char):
                key = json.dumps(program, sort_keys=True)
                if key in visited:
                    continue
                try:
                    for axis, nodes in enumerate(program):
                        hints.ordered(nodes, parent["graph"][char][axis])
                except ValueError:
                    continue
                candidates[key] = program
            # Hash order avoids always spending the bounded neighborhood on the
            # first axis. Each round/depth has a reproducible distinct ordering.
            keys = sorted(
                candidates, key=lambda k: sha(f"{round_index}:{level}:{k}".encode())
            )[:limit]
            for key in keys:
                visited.add(key)
                candidate = deepcopy(parent)
                candidate["programs"][char] = candidates[key]
                pool.append((oracle.score(candidate), candidate))
        pool.sort(
            key=lambda p: (p[0], json.dumps(p[1]["programs"][char], sort_keys=True))
        )
        frontier = pool[:width]
        if frontier[0][0] < best[0] - 1e-12:
            best = frontier[0]
    return best[1]


def accepts(before, after):
    """One atomic model check; no per-glyph reselection using reserved pixels."""
    old, new = means(before), means(after)
    return old.keys() == new.keys() and all(new[k] <= old[k] + 1e-12 for k in old)


def additional_data(directories, printer, glyphs, forbidden):
    """Read only extra development pixels and reject reuse of any held-out query."""
    references, provenance = {}, []
    for directory in directories:
        pages, source = pipeline.load_pages(directory, "development")
        require(source["printer"] == printer, "additional capture environment differs")
        captured = pipeline.cases(pages)
        for page, _ in pages:
            for p in page["probes"]:
                q = (
                    p["width"] or p["height"],
                    p["height"],
                    ord(p["text"]),
                    "NRIB".index(p["orientation"]),
                )
                ax, ay = p["anchor"]
                _, _, width, height = p["tile"]
                require(
                    all(
                        1 < x + ax < width - 2 and 1 < y + ay < height - 2
                        for x, y in captured[q]
                    ),
                    "additional reference touches tile boundary",
                )
        require(
            {chr(q[2]) for q in captured} == set(glyphs),
            "additional capture glyph coverage differs",
        )
        require(
            not (set(captured) & (set(references) | set(forbidden))),
            "additional capture duplicates or reuses a reserved query",
        )
        references.update(captured)
        provenance.append(source)
    return references, provenance


def frozen(root):
    data = (root / "model.json").read_bytes()
    model = json.loads(data)
    require(
        model["schema"] == SCHEMA and set(model["artifacts"]) == ARTIFACTS,
        "invalid joint model",
    )
    require(
        sha((root / "inputs.json").read_bytes()) == model["inputs_sha256"],
        "joint inputs changed",
    )
    progress = json.loads((root / "progress.json").read_text())
    require(
        progress["status"] == "complete" and progress["model_sha256"] == sha(data),
        "joint model is not frozen",
    )
    for name, digest in model["artifacts"].items():
        require(sha((root / name).read_bytes()) == digest, "frozen font changed")
    require(
        hints.build(model["state"]) == (root / "font.ttf").read_bytes(),
        "joint font does not reproduce",
    )
    require(
        hints.build(model["proposal"]) == (root / "proposal.ttf").read_bytes(),
        "joint proposal does not reproduce",
    )
    require(
        model["shapes"] == model["state"]["shapes"], "joint outline metadata differs"
    )
    return model, sha(data)


def fit(
    seed,
    source,
    development,
    output,
    binary,
    resume=False,
    budget=10000,
    rounds=3,
    beam_width=2,
    beam_depth=2,
    neighbors=64,
    extra_development=(),
    extra_source=(),
    reserved=(),
):
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
        legacy, seed_digest = pipeline.frozen(seed)
        require(
            legacy["schema"] == pipeline.SCHEMA,
            "joint search requires an automatic reconstruction seed",
        )
        baseline = hints.initialize(legacy)
        large_pages, source_info = pipeline.load_pages(source, "development")
        small_pages, small_info = pipeline.load_pages(development, "development")
        seed_inputs = json.loads((seed / "inputs.json").read_text())
        require(
            source_info == seed_inputs["source"]
            and small_info == seed_inputs["development"],
            "seed and joint search must use the same development captures",
        )
        large, small = pipeline.cases(large_pages), pipeline.cases(small_pages)
        train, check = partition(large, small)
        # Preserve the seed's original internal split. New configurations receive
        # their own split, so adding data cannot move an old check into training.
        forbidden = set(large) | set(small)
        for directory in reserved:
            manifest = json.loads((directory / "manifest.json").read_text())
            for page in manifest["pages"]:
                if page["group"] != "validation" or page.get("font", "0") != "0":
                    continue
                for p in page["probes"]:
                    forbidden.add(
                        (
                            p["width"] or p["height"],
                            p["height"],
                            ord(p["text"]),
                            "NRIB".index(p["orientation"]),
                        )
                    )
        extra_large, extra_large_info = additional_data(
            extra_source, source_info["printer"], baseline["shapes"], forbidden
        )
        require(
            all(category(q) == "large" for q in extra_large),
            "extra source requires large geometry cases",
        )
        extra_small, extra_small_info = additional_data(
            extra_development,
            source_info["printer"],
            baseline["shapes"],
            forbidden | set(extra_large),
        )
        require(
            all(category(q) != "large" for q in extra_small),
            "extra development contains large geometry cases",
        )
        if extra_small:
            more_train, more_check = partition(extra_large, extra_small)
            train.update(more_train)
            check.update(more_check)
        else:
            train.update(extra_large)
        large.update(extra_large)
        engine = pipeline.Engine(binary, output / "cache", budget)
        require(engine.digest == seed_inputs["engine_sha256"], "seed engine differs")
        scripts = set(seed_inputs["scripts"]) | {
            Path(__file__).name,
            Path(geometry.__file__).name,
            Path(hints.__file__).name,
        }
        seal = dict(
            schema=SCHEMA,
            source=source_info,
            development=small_info,
            extra_development=extra_small_info,
            extra_source=extra_large_info,
            reserved_manifests=[
                dict(
                    capture=p.name,
                    manifest_sha256=sha((p / "manifest.json").read_bytes()),
                )
                for p in reserved
            ],
            seed_sha256=seed_digest,
            engine_sha256=engine.digest,
            scripts={
                name: sha(Path(__file__).with_name(name).read_bytes())
                for name in sorted(scripts)
            },
            search=dict(
                rounds=rounds,
                beam_width=beam_width,
                beam_depth=beam_depth,
                neighbors=neighbors,
                geometry_steps=[4, 2, 1],
                numeric_steps=[8, 4, 2],
                geometry_proposals=24,
            ),
            objective=dict(
                worst_group=0.2, program=1e-5, coordinates=1e-6, shared_parameters=1e-7
            ),
            selection="development-only; preserve seed internal split, split additional configurations separately; final atomic acceptance once",
        )
        if resume:
            require(
                json.loads((output / "inputs.json").read_text()) == seal,
                "resume inputs, scripts, or engine changed",
            )
        else:
            save(output / "inputs.json", seal)
        save(output / "progress.json", progress)
        oracle = Oracle(engine, train, baseline)
        state = deepcopy(baseline)
        guides = {char: geometry.Guide(large, ord(char)) for char in state["shapes"]}

        def record(stage, before, **details):
            progress["stages"].append(
                dict(
                    stage=stage,
                    before=before,
                    after=oracle.score(state),
                    group_errors=means(oracle.errors(state)),
                    **details,
                )
            )
            progress["unique_evaluations"] = len(engine.seen)
            save(output / "progress.json", progress)
            print(json.dumps(progress["stages"][-1]), flush=True)

        for iteration in range(rounds):
            step = seal["search"]["geometry_steps"][min(iteration, 2)]
            for char in sorted(state["shapes"]):
                before = oracle.score(state)
                best, best_score, move = state, before, None
                for axis, indices, delta in guides[char].proposals(
                    state["shapes"][char], state["graph"][char], step, 24
                ):
                    shape = geometry.moved(
                        state["shapes"][char],
                        baseline["shapes"][char],
                        axis,
                        indices,
                        delta,
                    )
                    if shape is None:
                        continue
                    candidate = deepcopy(state)
                    candidate["shapes"][char] = shape
                    score = oracle.score(candidate)
                    if score < best_score - 1e-12:
                        best, best_score, move = (
                            candidate,
                            score,
                            [axis, indices, delta],
                        )
                state = best
                record("geometry", before, iteration=iteration, glyph=char, move=move)
            before = oracle.score(state)
            best, best_score, move = state, before, None
            step = seal["search"]["numeric_steps"][min(iteration, 2)]
            for candidate, label in numeric_neighbors(state, baseline, step):
                score = oracle.score(candidate)
                if score < best_score - 1e-12:
                    best, best_score, move = candidate, score, label
            state = best
            record("shared-parameters", before, iteration=iteration, move=move)
            for char in sorted(state["shapes"]):
                before = oracle.score(state)
                state = beam(
                    state, char, oracle, beam_width, beam_depth, neighbors, iteration
                )
                record("hint-program", before, iteration=iteration, glyph=char)
        proposal = deepcopy(state)
        # Neither beam selection nor earlier geometry/CVT search has read these
        # pixels through an oracle. This gate is called exactly once at the end.
        acceptance = Oracle(engine, check, baseline)
        before, after = acceptance.floor, acceptance.errors(proposal)
        accepted = accepts(before, after)
        if not accepted:
            state = deepcopy(baseline)
        model = dict(
            schema=SCHEMA,
            inputs_sha256=sha((output / "inputs.json").read_bytes()),
            state=state,
            proposal=proposal,
            shapes=state["shapes"],
            training_queries=sorted(set(train) | set(check)),
            search_queries=sorted(train),
            internal_check_queries=sorted(check),
            acceptance=dict(
                accepted=accepted, before=means(before), after=means(after)
            ),
            training=dict(
                before=means(oracle.floor),
                proposal=means(oracle.errors(proposal)),
                before_objective=oracle.score(baseline),
                proposal_objective=oracle.score(proposal),
            ),
            spacing=legacy["spacing"],
            production_ready=False,
        )
        for name, data in {
            "initial.ttf": hints.build(baseline),
            "font.ttf": hints.build(state),
            "proposal.ttf": hints.build(proposal),
            "geometry.ttf": pipeline.engine_api.build_font(state["shapes"]),
        }.items():
            (output / name).write_bytes(data)
        model["artifacts"] = {
            name: sha((output / name).read_bytes()) for name in sorted(ARTIFACTS)
        }
        save(output / "model.json", model)
        progress.update(
            status="complete",
            unique_evaluations=len(engine.seen),
            executed_evaluations=engine.executed,
            model_sha256=sha((output / "model.json").read_bytes()),
        )
        save(output / "progress.json", progress)
        print(
            json.dumps(
                dict(
                    status="complete",
                    evaluations=len(engine.seen),
                    acceptance=model["acceptance"],
                    training=model["training"],
                )
            ),
            flush=True,
        )
    except BaseException as error:
        progress.update(status="failed", error=str(error))
        save(output / "progress.json", progress)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "source", "development", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--max-evaluations", type=int, default=10000)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--beam-width", type=int, default=2)
    parser.add_argument("--beam-depth", type=int, default=2)
    parser.add_argument("--neighbors", type=int, default=64)
    parser.add_argument("--extra-development", nargs="*", type=Path, default=[])
    parser.add_argument("--extra-source", nargs="*", type=Path, default=[])
    parser.add_argument(
        "--reserved",
        nargs="*",
        type=Path,
        default=[],
        help="exclude these validation manifests without opening their pixels",
    )
    args = parser.parse_args()
    try:
        require(
            1 <= args.max_evaluations <= 50000
            and 1 <= args.rounds <= 8
            and 1 <= args.beam_width <= 4
            and 1 <= args.beam_depth <= 4
            and 1 <= args.neighbors <= 256,
            "search bounds exceeded",
        )
        fit(
            args.seed,
            args.source,
            args.development,
            args.output,
            args.engine,
            args.resume,
            args.max_evaluations,
            args.rounds,
            args.beam_width,
            args.beam_depth,
            args.neighbors,
            args.extra_development,
            args.extra_source,
            args.reserved,
        )
    except (ValueError, OSError) as error:
        parser.exit(1, f"optimize_font: {error}\n")


if __name__ == "__main__":
    main()
