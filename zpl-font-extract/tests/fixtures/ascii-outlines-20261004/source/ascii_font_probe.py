"""Predeclare printable-ASCII geometry, hint and spacing observations.

Existing six-glyph captures retain their original partitions. New glyphs receive
large outline sources, dense small sizes and transforms. Independent validation
configurations are selected using request metadata only, before capture.
Space is measured by sentinel displacement, never by blank-pixel agreement.
Zebra ^FT/^A0/^FH: ZPL Programming Guide command reference, linked in
reconstruction_probe.py. All requests use the native printer canvas.
"""

import argparse
import json
import math
from pathlib import Path

from capture_font_probe import sha
from font0_outline_probe import capture as capture_campaign
from reconstruct_font import require, save
from reconstruction_probe import prepare as prepare_glyphs

PRINTABLE = "".join(map(chr, range(32, 127)))
VISIBLE = PRINTABLE[1:]
NEW_GLYPHS = "".join(c for c in VISIBLE if c not in "HOSgj@")
PREFIX = "^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"


def capacity(configurations):
    size = max(max(w, h) for w, h, _ in configurations)
    padding = 16 if size <= 64 else math.ceil(size / 5) + 16
    tile = math.ceil((size + 2 * padding) / 32) * 32
    width = 384 if tile <= 128 else 768
    return (width // tile) * (1536 // tile)


def prepare_spacing(root, characters, sizes, group):
    require(characters and set(characters) <= set(PRINTABLE), "invalid spacing glyphs")
    require(sizes and all(1 <= s <= 256 for s in sizes), "invalid spacing sizes")
    size = max(sizes)
    tw = math.ceil((2 * size + 64) / 64) * 64
    th = math.ceil((size * 4 / 3 + 32) / 32) * 32
    columns, per_page = 768 // tw, (768 // tw) * (1536 // th)
    fields = [(s, c) for s in sizes for c in characters]
    # Repeat the empty-pair reference on every page, at every requested size.
    per_page -= len(sizes)
    require(per_page > 0, "spacing reference exceeds page budget")
    require(math.ceil(len(fields) / per_page) + 3 <= 32, "spacing preview budget")
    root.mkdir(parents=True, exist_ok=False)
    pages = []
    for start in range(0, len(fields), per_page):
        chunk = [(s, None) for s in sizes] + fields[start : start + per_page]
        height = math.ceil(len(chunk) / columns) * th
        zpl = f"^XA^PW768^LL{height}" + PREFIX
        probes = []
        for i, (size, char) in enumerate(chunk):
            tx, ty = i % columns * tw, i // columns * th
            text = "|" + (char or "") + "|"
            ax, ay = 16, 16 + size
            probes.append(
                dict(
                    text=text,
                    character=char,
                    width=size,
                    height=size,
                    orientation="N",
                    origin="FT",
                    tile=[tx, ty, tw, th],
                    anchor=[ax, ay],
                )
            )
            encoded = "".join(f"_{ord(c):02X}" for c in text)
            zpl += f"^FT{tx+ax},{ty+ay}^A0N,{size},{size}^FH_^FD{encoded}^FS"
        data = (zpl + "^XZ").encode()
        name = f"spacing-{len(pages):02}"
        (root / (name + ".zpl")).write_bytes(data)
        pages.append(
            dict(
                name=name,
                font="0",
                group=group,
                canvas=[768, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )
    save(
        root / "manifest.json",
        dict(
            schema="small-font-probe-v1",
            kind="spacing",
            characters=characters,
            sizes=sizes,
            pages=pages,
        ),
    )


def prepare(root, fixtures):
    require(not root.exists(), "sampling directory already exists")
    previous, excluded = [], set()
    for path in sorted(fixtures.rglob("manifest.json")):
        data = path.read_bytes()
        manifest = json.loads(data)
        if manifest.get("schema") not in (
            "small-font-probe-v1",
            "font0-outline-pilot-v1",
        ):
            continue
        previous.append(dict(path=str(path.relative_to(fixtures)), sha256=sha(data)))
        for page in manifest["pages"]:
            if page.get("font", "0") == "0":
                excluded.update(
                    (p["width"] or p["height"], p["height"], p["orientation"])
                    for p in page["probes"]
                )
    root.mkdir(parents=True)
    campaigns = []

    def record(name, group, role):
        data = (root / name / "manifest.json").read_bytes()
        manifest = json.loads(data)
        campaigns.append(
            dict(
                name=name,
                group=group,
                role=role,
                manifest_sha256=sha(data),
                previews=len(manifest["pages"]) + 3,
                cases=sum(
                    p.get("character", p["text"]) is not None
                    for page in manifest["pages"]
                    for p in page["probes"]
                ),
            )
        )

    def glyphs(characters, configs, group, role):
        chunk_size = min(len(characters), 29 * capacity(configs) // len(configs))
        require(chunk_size > 0, "configuration batch exceeds preview budget")
        for offset in range(0, len(characters), chunk_size):
            name = f"{group}-{role}-{len(campaigns):02}"
            prepare_glyphs(
                root / name, characters[offset : offset + chunk_size], configs, group
            )
            record(name, group, role)

    for size in (192, 384):
        glyphs(NEW_GLYPHS, [(size, size, "N")], "development", "outline")
    training = [(s, s, "N") for s in (*range(10, 33), 36, 40, 48, 64, 80, 96)]
    training += [
        (w, h, "N")
        for w, h in (
            (12, 18),
            (18, 12),
            (16, 24),
            (24, 16),
            (24, 32),
            (32, 24),
            (32, 48),
            (48, 32),
        )
    ]
    training += [(s, s, r) for s in (10, 16, 24, 40) for r in "RIB"]
    for low, high in ((0, 32), (32, 64), (64, 96)):
        selected = [q for q in training if low < max(q[:2]) <= high]
        for offset in range(0, len(selected), 24):
            glyphs(NEW_GLYPHS, selected[offset : offset + 24], "development", "hints")
    excluded.update(training)
    # Hash ordering is fixed before seeing any pixels; cover each transformation.
    validation = []
    for rotation in "NRIB":
        candidates = [
            (w, h, rotation)
            for w in range(11, 50, 2)
            for h in range(13, 50, 2)
            if 0.6 <= w / h <= 1.6 and (w, h, rotation) not in excluded
        ]
        candidates.sort(key=lambda q: sha(json.dumps(q).encode()))
        require(len(candidates) >= 4, "insufficient unseen validation sizes")
        validation += candidates[:4]
    glyphs(VISIBLE, validation, "validation", "hints")
    large = next(s for s in (272, 288, 336, 368) if (s, s, "N") not in excluded)
    glyphs(VISIBLE, [(large, large, "N")], "validation", "outline")
    for group, sizes in (
        ("development", (12, 16, 24, 40, 80, 192)),
        ("validation", (19, 37, 112)),
    ):
        for size in sizes:
            name = f"{group}-spacing-{size}"
            prepare_spacing(root / name, PRINTABLE, [size], group)
            record(name, group, "spacing")
    save(
        root / "plan.json",
        dict(
            schema="ascii-font-sampling-v1",
            target_iou=0.9,
            characters=PRINTABLE,
            new_outline_characters=NEW_GLYPHS,
            space_target="exact measured advance; evaluate strings at their original origin",
            previous_manifests=previous,
            campaigns=campaigns,
            validation_capture_after_candidate_freeze=True,
            previews=sum(c["previews"] for c in campaigns),
        ),
    )


def capture(root, group, host, roles=None):
    plan = json.loads((root / "plan.json").read_text())
    require(plan["schema"] == "ascii-font-sampling-v1", "unknown sampling plan")
    for c in plan["campaigns"]:
        if c["group"] != group or (roles and c["role"] not in roles):
            continue
        directory = root / c["name"]
        require(
            sha((directory / "manifest.json").read_bytes()) == c["manifest_sha256"],
            "sampling plan changed",
        )
        record = directory / "zd621/capture.json"
        if record.exists():
            require(
                json.loads(record.read_text())["status"] == "complete",
                "failed campaign requires diagnosis, no automatic retry",
            )
            continue
        print(c["name"], flush=True)
        capture_campaign(directory, host, schema="small-font-probe-v1")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "capture"))
    parser.add_argument("root", type=Path)
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument(
        "--group", choices=("development", "validation"), default="development"
    )
    parser.add_argument("--roles", nargs="+", choices=("outline", "hints", "spacing"))
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    args = parser.parse_args()
    if args.action == "prepare":
        require(args.fixtures is not None, "prepare requires fixture metadata")
        prepare(args.root, args.fixtures)
    else:
        capture(args.root, args.group, args.host, args.roles)
