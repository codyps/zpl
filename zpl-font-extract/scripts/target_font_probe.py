"""Reserve unseen native Font 0 cases for auditing the 90% fitting target.

Preparation reads request manifests only. Capture is a separate action performed
after candidate freeze, using existing identity checks and repeated controls.
"""

import argparse
import json
from pathlib import Path

from capture_font_probe import sha
from font0_outline_probe import capture
from reconstruct_font import require, save
from reconstruction_probe import prepare as prepare_campaign


def prepare(fixtures, output, minimum_controls=False):
    excluded, manifests = set(), []
    for path in sorted(fixtures.rglob("manifest.json")):
        data = path.read_bytes()
        manifest = json.loads(data)
        if manifest.get("schema") not in (
            "font0-outline-pilot-v1",
            "small-font-probe-v1",
        ):
            continue
        manifests.append(dict(path=str(path.relative_to(fixtures)), sha256=sha(data)))
        for page in manifest["pages"]:
            if page.get("font", "0") != "0":
                continue
            excluded.update(
                (p["width"] or p["height"], p["height"], p["orientation"])
                for p in page["probes"]
            )
    choices = {
        "small": (
            20,
            [
                (w, h, "N")
                for w in range(11, 33)
                for h in range(11, 33)
                if abs(w - h) <= 4
            ],
        ),
        "stretched": (
            20,
            [
                (w, h, "N")
                for w in range(11, 65, 2)
                for h in range(11, 65, 2)
                if 33 <= max(w, h) <= 64 and 1.4 <= max(w, h) / min(w, h) <= 3
            ],
        ),
        "axis-guard": (
            12,
            [
                (w, h, "N")
                for long in (96, 112, 128)
                for short in (11, 15, 19, 23, 27)
                for w, h in ((long, short), (short, long))
            ],
        ),
        "rotated": (
            12,
            [
                (w, h, r)
                for w in range(11, 42, 4)
                for h in range(13, 42, 4)
                for r in "RIB"
            ],
        ),
    }
    if minimum_controls:
        choices = {
            "minimum-clamp": (
                12,
                [(w, h, r) for w in range(1, 11) for h in range(1, 11) for r in "NRIB"],
            )
        }
    output.mkdir(parents=True, exist_ok=False)
    campaigns = []
    for family, (count, options) in choices.items():
        options = sorted(
            set(options) - excluded, key=lambda q: sha(json.dumps([family, q]).encode())
        )
        require(len(options) >= count, "not enough unseen configurations")
        selected = options[:count]
        excluded.update(selected)
        root = output / family
        prepare_campaign(root, "HOSgj@", selected, "validation")
        campaigns.append(
            dict(
                path=family,
                configurations=selected,
                manifest_sha256=sha((root / "manifest.json").read_bytes()),
            )
        )
    save(
        output / "plan.json",
        dict(
            schema="target-font-probe-v1",
            target_iou=0.9,
            scope="every case, plus pooled glyph/size/transform strata",
            cases=sum(len(c["configurations"]) * 6 for c in campaigns),
            role=(
                "minimum-clamp controls; nominal sizes differ, effective scale is shared"
                if minimum_controls
                else "fresh size/rotation validation"
            ),
            excluded_manifests=manifests,
            campaigns=campaigns,
            capture_after_candidates_frozen=True,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "capture"))
    parser.add_argument("root", type=Path)
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument("--minimum-controls", action="store_true")
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    args = parser.parse_args()
    if args.action == "prepare":
        require(args.fixtures is not None, "prepare requires fixtures")
        prepare(args.fixtures, args.root, args.minimum_controls)
    else:
        plan = json.loads((args.root / "plan.json").read_text())
        require(plan["schema"] == "target-font-probe-v1", "unknown capture plan")
        for campaign in plan["campaigns"]:
            root = args.root / campaign["path"]
            require(
                sha((root / "manifest.json").read_bytes())
                == campaign["manifest_sha256"],
                "capture plan changed",
            )
            capture(root, args.host, schema="small-font-probe-v1")
