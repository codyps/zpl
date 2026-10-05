"""Merge frozen glyph fits without changing their geometry or hint behavior."""

import argparse
from copy import deepcopy
import json
from pathlib import Path

import joint_hint_program as hints
from reconstruct_font import require, save, sha
from ascii_font_probe import PRINTABLE, VISIBLE
from ascii_font_metrics import layout_advance
import reconstruct_font as pipeline


def merge(first, second):
    require(
        not (set(first["shapes"]) & set(second["shapes"])), "overlapping glyph models"
    )
    a, b = first["parameters"], second["parameters"]
    require(a["limit"] == b["limit"], "shared hint size guards differ")
    state = deepcopy(first)
    widths = []
    for axis in (0, 1):
        values = state["parameters"]["widths"][axis]
        mapping = []
        for value in b["widths"][axis]:
            if value not in values:
                values.append(value)
            mapping.append(values.index(value))
        widths.append(mapping)
    zones = []
    for origin, value in zip(second["zone_origins"], b["zones"], strict=True):
        pairs = list(
            zip(state["zone_origins"], state["parameters"]["zones"], strict=True)
        )
        if (origin, value) not in pairs:
            state["zone_origins"].append(origin)
            state["parameters"]["zones"].append(value)
            pairs.append((origin, value))
        zones.append(pairs.index((origin, value)))

    def remap(nodes, axis):
        if nodes is None:
            return None
        result = deepcopy(nodes)
        for node in result:
            if node:
                if node.get("stem") is not None:
                    node["stem"] = widths[axis][node["stem"]]
                    if a["cutin"] != b["cutin"]:
                        node.setdefault("cutin", b["cutin"])
                if "zone" in node:
                    node["zone"] = zones[node["zone"]]
        return result

    def regime(record, axis):
        if record is None:
            return None
        result = deepcopy(record)
        result["nodes"] = remap(record["nodes"], axis)
        if "smaller" in record:
            result["smaller"] = regime(record["smaller"], axis)
        return result

    for c in second["shapes"]:
        if c in second.get("diagonal_programs", {}):
            state.setdefault("diagonal_programs", {})[c] = deepcopy(
                second["diagonal_programs"][c]
            )
        state["shapes"][c] = deepcopy(second["shapes"][c])
        state["graph"][c] = deepcopy(second["graph"][c])
        state["programs"][c] = [
            remap(nodes, axis) for axis, nodes in enumerate(second["programs"][c])
        ]
        if c in second.get("regimes", {}):
            state.setdefault("regimes", {})[c] = [
                regime(r, axis) for axis, r in enumerate(second["regimes"][c])
            ]
        if c in second.get("optical_programs", {}):
            optical = deepcopy(second["optical_programs"][c])
            optical["nodes"] = [
                remap(nodes, axis) for axis, nodes in enumerate(optical["nodes"])
            ]
            state.setdefault("optical_programs", {})[c] = optical
    if "independent_axes" in first or "independent_axes" in second:
        state["independent_axes"] = sorted(
            set(first.get("independent_axes", []))
            | set(second.get("independent_axes", []))
        )
    state["advances"] = dict(first.get("advances", {}), **second.get("advances", {}))
    return state


def freeze(six, ascii_fit, output, engine):
    six_report = json.loads((six / "report.json").read_text())
    six_data = (six / "candidates.json").read_bytes()
    require(sha(six_data) == six_report["candidates_sha256"], "six-glyph fit changed")
    six_states = json.loads(six_data)
    ascii_report = json.loads((ascii_fit / "report.json").read_text())
    ascii_data = (ascii_fit / "state.json").read_bytes()
    require(sha(ascii_data) == ascii_report["state_sha256"], "ASCII fit changed")
    ascii_state = json.loads(ascii_data)
    require(
        sha(hints.build(ascii_state)) == ascii_report["font_sha256"],
        "ASCII font changed",
    )
    ascii_baseline = (
        json.loads((ascii_fit / "baseline.json").read_text())
        if (ascii_fit / "baseline.json").exists()
        else ascii_state
    )
    states = {}
    for label, part in (("baseline", ascii_baseline), ("targeted", ascii_state)):
        require(
            sha(hints.build(six_states[label]))
            == six_report["variants"][label]["font_sha256"],
            "six-glyph compiler output changed",
        )
        states[label] = merge(six_states[label], part)
        require(
            set(states[label]["shapes"]) == set(VISIBLE), "incomplete ASCII outlines"
        )
        require(
            set(states[label]["advances"]) == set(PRINTABLE),
            "incomplete ASCII advances",
        )
    output.mkdir(parents=True, exist_ok=False)
    plans = [json.loads((root / "plan.json").read_text()) for root in (six, ascii_fit)]
    require(
        plans[0]["engine_sha256"] == plans[1]["engine_sha256"], "fit engines differ"
    )
    require(
        sha(engine.read_bytes()) == plans[0]["engine_sha256"],
        "merge audit engine differs",
    )
    if (ascii_fit / "baseline.json").exists():
        require(
            sha((ascii_fit / "baseline.json").read_bytes()) == plans[1]["seed_sha256"],
            "ASCII baseline changed",
        )
    source = output / "source"
    source.mkdir()
    scripts = {}
    for name in (
        "merge_ascii_font",
        "joint_hint_program",
        "font_probe",
        "ascii_font_metrics",
        "reconstruct_font",
    ):
        path = Path(__file__).with_name(name + ".py")
        data = path.read_bytes()
        (source / path.name).write_bytes(data)
        scripts[path.name] = sha(data)
    save(
        output / "plan.json",
        dict(
            schema="structured-font-fit-v1",
            engine_sha256=plans[0]["engine_sha256"],
            printer=plans[0]["printer"],
            source_reports={
                str(root): sha((root / "report.json").read_bytes())
                for root in (six, ascii_fit)
            },
            search_queries=sorted(
                plans[0]["search_queries"] + plans[1]["search_queries"]
            ),
            internal_check_queries=sorted(
                plans[0]["internal_check_queries"] + plans[1]["internal_check_queries"]
            ),
            scope="all 95 printable ASCII characters, including measured space advance",
            scripts=scripts,
        ),
    )
    save(output / "candidates.json", states)
    for label, state in states.items():
        (output / (label + ".ttf")).write_bytes(hints.build(state))
    runner = pipeline.Engine(engine, output / "merge-cache", 8)
    keys = [
        sorted(set(map(tuple, p["search_queries"] + p["internal_check_queries"])))
        for p in plans
    ]
    require(not (set(keys[0]) & set(keys[1])), "component queries overlap")
    for label, state in states.items():
        rows = runner.render(hints.build(state), sorted(keys[0] + keys[1]))
        parts = [
            six_states[label],
            ascii_baseline if label == "baseline" else ascii_state,
        ]
        for part, queries in zip(parts, keys, strict=True):
            separate = runner.render(hints.build(part), queries)
            require(
                all(
                    pipeline.pixels(rows[q]) == pipeline.pixels(separate[q])
                    for q in queries
                ),
                "font assembly changed component rasters",
            )
        require(
            all(
                row["layout_advance"]
                == layout_advance(state["advances"][chr(q[2])], q[0])
                for q, row in rows.items()
            ),
            "font assembly changed measured advances",
        )
    save(
        output / "report.json",
        dict(
            schema="structured-font-fit-v1",
            designated="targeted",
            production_ready=False,
            component_raster_audit_exact=True,
            component_raster_audit_cases=sum(map(len, keys)),
            candidates_sha256=sha((output / "candidates.json").read_bytes()),
            plan_sha256=sha((output / "plan.json").read_bytes()),
            variants={
                label: dict(font_sha256=sha(hints.build(state)))
                for label, state in states.items()
            },
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("six", "ascii_fit", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--engine", type=Path, required=True)
    args = parser.parse_args()
    freeze(args.six, args.ascii_fit, args.output, args.engine)
