"""Original bounded SFNT/probe generator for the October 2026 printer experiment.

Table formats: https://learn.microsoft.com/en-us/typography/opentype/spec/otff
Instructions: https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
The existing original calibration font supplies unchanged ancillary table defaults.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = (
    ROOT / "zebra-http-api/tests/fixtures/font-refinement-v1/calibration-plain.ttf"
)
OBJECT = "R:ZP26A.TTF"


def rect(x, y, w, h):
    return [(x, y, True), (x, y + h, True), (x + w, y + h, True), (x + w, y, True)]


def push(n):
    return bytes([0xB0, n]) if 0 <= n < 256 else bytes([0xB8]) + struct.pack(">h", n)


def set_right(expression):
    # Right edge points 2 and 3; SCFS consumes point index, then F26Dot6 value.
    return b"".join(push(p) + expression + bytes([0x01, 0x48]) for p in (2, 3))


def glyphs():
    out = []

    def add(name, contours, instructions=b"", advance=1024, family="geometry"):
        out.append(
            dict(
                name=name,
                codepoint=32 + len(out),
                contours=contours,
                instructions=instructions.hex(),
                advance=advance,
                family=family,
            )
        )

    add("space", [], advance=512, family="unused")
    add(
        "identity",
        [rect(0, 0, 128, 512), rect(256, 0, 128, 768), rect(512, 0, 256, 1024)],
        family="identity",
    )
    for phase in (0, 16, 31, 32, 33, 48, 63):
        for kind, shape in (
            ("vertical-wide", rect(256 + phase, 0, 128, 1024)),
            ("horizontal-wide", rect(256, 128 + phase, 1024, 128)),
            ("vertical-thin", rect(256 + phase, 0, 32, 1024)),
            ("horizontal-thin", rect(256, 128 + phase, 1024, 32)),
        ):
            add(f"{kind}-{phase}", [shape], family="phase")
        add(
            f"diagonal-{phase}",
            [
                [
                    (256 + phase, 0, True),
                    (1024 + phase, 1024, True),
                    (1056 + phase, 1024, True),
                    (288 + phase, 0, True),
                ]
            ],
        )
    for phase in (0, 32, 33):
        # Quadratic A=(256,1024), B=(1280,1024), C=(768,0).
        # Exact de Casteljau split uses D=(768,1024), E=(1024,512), M=(896,768).
        start = [(256 + phase, 0, True), (256 + phase, 1024, True)]
        add(
            f"curve-whole-{phase}",
            [start + [(1280 + phase, 1024, False), (768 + phase, 0, True)]],
        )
        add(
            f"curve-split-{phase}",
            [
                start
                + [
                    (768 + phase, 1024, False),
                    (896 + phase, 768, True),
                    (1024 + phase, 512, False),
                    (768 + phase, 0, True),
                ]
            ],
        )
    add("overlap", [rect(256, 0, 768, 1024), rect(640, 512, 768, 1024)])
    add(
        "counter", [rect(256, 0, 1024, 1536), list(reversed(rect(512, 256, 512, 1024)))]
    )
    add("stub-isolated", [rect(288, 256, 32, 32)])
    add("stub-connected", [rect(288, 256, 32, 256), rect(256, 512, 128, 128)])
    # A fixed-width execution witness, then interpreter state encoded as width.
    add(
        "instruction-witness",
        [rect(0, 0, 1024, 256)],
        set_right(push(13 * 64)),
        family="state",
    )
    for axis, op in [("x", 0x01), ("y", 0x00)]:
        expression = bytes([op, 0x4B]) + push(4096) + bytes([0x63])
        add(
            f"ppem-{axis}",
            [rect(0, 0, 1024, 256)],
            set_right(expression),
            family="state",
        )
    for name, selector in [("rotation", 2), ("stretch", 4), ("grayscale", 32)]:
        # Query one GETINFO flag, convert nonzero to Boolean, encode 8 or 12 dots.
        expression = (
            push(selector)
            + bytes([0x88])
            + push(0)
            + bytes([0x55])
            + push(16384)
            + bytes([0x63])
            + push(512)
            + bytes([0x60])
        )
        add(
            f"flag-{name}",
            [rect(0, 0, 1024, 256)],
            set_right(expression),
            family="state",
        )
    add("sentinel", [rect(0, 0, 128, 1024)], advance=384, family="metrics")
    for advance in (735, 736, 737):
        add(
            f"advance-{advance}",
            [rect(0, 0, 128, 512)],
            advance=advance,
            family="metrics",
        )
    # Four outline points, pp1 index 4 and advance-defining pp2 index 5.
    add(
        "advance-735-plus-one",
        [rect(0, 0, 128, 512)],
        bytes([0x01]) + push(5) + push(64) + bytes([0x38]),
        advance=735,
        family="metrics",
    )
    assert out[-1]["codepoint"] <= 126
    return out


def checksum(data):
    data += bytes((-len(data)) % 4)
    return sum(struct.unpack(f">{len(data)//4}I", data)) & 0xFFFFFFFF


def cmap4(items):
    """Map contiguous glyph IDs in sorted BMP segments, including sparse PUA.

    OpenType cmap format 4, idDelta with idRangeOffset=0:
    https://learn.microsoft.com/en-us/typography/opentype/spec/cmap#format-4-segment-mapping-to-delta-values
    The previous contiguous ASCII mapping retains exactly the same bytes.
    """
    codes = [g["codepoint"] for g in items]
    if (
        not codes
        or codes != sorted(set(codes))
        or not 0 <= codes[0] <= codes[-1] < 65535
    ):
        raise ValueError("cmap requires sorted unique BMP codepoints")
    segments = []
    for index, code in enumerate(codes, 1):
        delta = (index - code) & 65535
        if segments and code == segments[-1][1] + 1 and delta == segments[-1][2]:
            segments[-1][1] = code
        else:
            segments.append([code, code, delta])
    segments.append([65535, 65535, 1])
    count = len(segments)
    power = 1 << (count.bit_length() - 1)
    length = 16 + 8 * count
    if length > 65535:
        raise ValueError("cmap exceeds format 4 length")
    data = bytearray(struct.pack(">HHHHI", 0, 1, 3, 1, 12))
    data += struct.pack(
        ">7H",
        4,
        length,
        0,
        count * 2,
        power * 2,
        power.bit_length() - 1,
        count * 2 - power * 2,
    )
    data += struct.pack(">" + str(count) + "H", *(s[1] for s in segments))
    data += b"\0\0"
    for column in (0, 2):
        data += struct.pack(">" + str(count) + "H", *(s[column] for s in segments))
    data += bytes(count * 2)
    return data


def witness_codepoints(characters):
    return (0xE000, 0xE001) if set(characters) & set('!"') else (33, 34)


def add_witnesses(items, characters):
    identity, execution = witness_codepoints(characters)
    witnesses = [
        dict(glyphs()[1], codepoint=identity),
        dict(
            name="instruction-witness",
            codepoint=execution,
            contours=[rect(0, 0, 1024, 256)],
            instructions=set_right(push(13 * 64)).hex(),
            advance=1024,
        ),
    ]
    indexed = {g["codepoint"]: g for g in items}
    indexed.update({g["codepoint"]: g for g in witnesses})
    return [indexed[code] for code in sorted(indexed)]


def font(items, extra_tables=None):
    template = TEMPLATE.read_bytes()
    assert (
        hashlib.sha256(template).hexdigest()
        == "8d170e65a6109ef3dd16c30ef683809fae81342ccf59d496a9f210cfd4863e19"
    )
    tables = {}
    for i in range(struct.unpack_from(">H", template, 4)[0]):
        tag, _, offset, length = struct.unpack_from(">4sIII", template, 12 + 16 * i)
        tables[tag] = bytearray(template[offset : offset + length])
    glyf, loca, hmtx = bytearray(), bytearray(), bytearray()
    bounds, point_counts, contour_counts, instruction_lengths = [], [], [], []
    for g in [dict(contours=[], instructions="", advance=1024)] + items:
        loca += struct.pack(">I", len(glyf))
        points = [p for contour in g["contours"] for p in contour]
        instructions = bytes.fromhex(g["instructions"])
        point_counts.append(len(points))
        contour_counts.append(len(g["contours"]))
        instruction_lengths.append(len(instructions))
        box = (
            (
                min(p[0] for p in points),
                min(p[1] for p in points),
                max(p[0] for p in points),
                max(p[1] for p in points),
            )
            if points
            else (0, 0, 0, 0)
        )
        hmtx += struct.pack(">Hh", g["advance"], box[0])
        if not points:
            continue
        bounds.append(box)
        glyf += struct.pack(">hhhhh", len(g["contours"]), *box)
        end = 0
        for c in g["contours"]:
            end += len(c)
            glyf += struct.pack(">H", end - 1)
        glyf += struct.pack(">H", len(instructions)) + instructions
        glyf += bytes(int(p[2]) for p in points)
        for axis in (0, 1):
            previous = 0
            for p in points:
                glyf += struct.pack(">h", p[axis] - previous)
                previous = p[axis]
        glyf += bytes((-len(glyf)) % 4)
    loca += struct.pack(">I", len(glyf))
    tables.update(
        {
            b"glyf": glyf,
            b"loca": loca,
            b"hmtx": hmtx,
            b"prep": bytearray([0xB0, 0, 0x21]),
        }
    )
    head = tables[b"head"]
    struct.pack_into(">I", head, 8, 0)
    struct.pack_into(
        ">HH", head, 16, 23, 2048
    )  # baseline, lsb=xMin, size/advance hints
    box = (
        min(b[0] for b in bounds),
        min(b[1] for b in bounds),
        max(b[2] for b in bounds),
        max(b[3] for b in bounds),
    )
    struct.pack_into(">hhhh", head, 36, *box)
    hhea = tables[b"hhea"]
    struct.pack_into(">hhhHhhh", hhea, 4, 1536, -512, 0, 1024, box[0], 0, box[2])
    struct.pack_into(">H", hhea, 34, len(items) + 1)
    maxp = bytearray(32)
    struct.pack_into(
        ">I14H",
        maxp,
        0,
        0x10000,
        len(items) + 1,
        max(point_counts),
        max(contour_counts),
        0,
        0,
        1,
        0,
        0,
        0,
        0,
        16,
        max(instruction_lengths),
        0,
        0,
    )
    tables[b"maxp"] = maxp
    tables[b"cmap"] = cmap4(items)
    os2 = tables[b"OS/2"]
    struct.pack_into(
        ">HHhhhHH", os2, 64, 32, items[-1]["codepoint"], 1536, -512, 0, 1536, 512
    )
    struct.pack_into(">hh", os2, 86, 1024, 1536)
    records = [
        (1, "ZplProbe20261002"),
        (2, "Regular"),
        (4, "ZplProbe20261002 Regular"),
        (5, "Version 1.0"),
        (6, "ZplProbe20261002-Regular"),
    ]
    name, strings = (
        bytearray(struct.pack(">HHH", 0, len(records), 6 + 12 * len(records))),
        bytearray(),
    )
    for id_, text in records:
        data = text.encode("utf-16-be")
        name += struct.pack(">6H", 3, 1, 0x409, id_, len(data), len(strings))
        strings += data
    tables[b"name"] = name + strings
    if extra_tables:
        tables.update(extra_tables)
    n = len(tables)
    power = 1 << (n.bit_length() - 1)
    data = bytearray(
        struct.pack(
            ">I4H", 0x10000, n, power * 16, power.bit_length() - 1, n * 16 - power * 16
        )
    ) + bytearray(n * 16)
    for i, (tag, table) in enumerate(sorted(tables.items())):
        offset = len(data)
        struct.pack_into(
            ">4sIII", data, 12 + i * 16, tag, checksum(table), offset, len(table)
        )
        if tag == b"head":
            head_offset = offset
        data += table + bytes((-len(table)) % 4)
    struct.pack_into(
        ">I", data, head_offset + 8, (0xB1B0AFBA - checksum(data)) & 0xFFFFFFFF
    )
    assert checksum(data) == 0xB1B0AFBA
    return bytes(data)


def plan(items):
    pages = []
    by_name = {g["name"]: g for g in items}
    identity = by_name["identity"]

    def add(group, h, w=0, orientation="N", selected=None, texts=None):
        side = math.ceil((max(h, w) + 32) / 16) * 16
        probes = []
        if texts is None:
            selected = [identity] + [g for g in selected if g != identity]
            cells = [(chr(g["codepoint"]), g["name"], "FT", side) for g in selected]
        else:
            cells = texts
        x = y = 0
        for text, name, origin, cw in cells:
            if x + cw > 384:
                x = 0
                y += side
            if y + side > 1024:
                raise ValueError("Probe page exceeds native height budget")
            ax, ay = 16, 16 + h
            ax, ay = {
                "N": (ax, ay),
                "R": (side - ay, ax),
                "I": (side - ax, side - ay),
                "B": (ay, side - ax),
            }[orientation]
            if origin == "FO":
                ax = ay = 16
            probes.append(
                dict(
                    text=text,
                    name=name,
                    origin=origin,
                    tile=[x, y, cw, side],
                    anchor=[ax, ay],
                    height=h,
                    width=w,
                    orientation=orientation,
                    font=OBJECT,
                )
            )
            x += cw
        pages.append(
            dict(
                name=f"{len(pages):02}-{group}",
                group=group,
                canvas=[384, y + side],
                probes=probes,
            )
        )

    for h, w, t in [
        (16, 0, "N"),
        (32, 0, "N"),
        (64, 0, "N"),
        (32, 19, "N"),
        (32, 64, "N"),
        (32, 19, "R"),
        (32, 19, "I"),
        (32, 19, "B"),
    ]:
        add("state", h, w, t, [g for g in items if g["family"] == "state"])
    for h, w, t in [(32, 0, t) for t in "NRIB"] + [
        (31, 0, "N"),
        (33, 0, "N"),
        (64, 0, "N"),
        (32, 19, "N"),
    ]:
        selected = [g for g in items if g["family"] in ("phase", "geometry")]
        if h != 32:
            selected = [g for g in selected if g["family"] == "phase"]
        add("geometry", h, w, t, selected)
    sentinel = chr(by_name["sentinel"]["codepoint"])
    for h in (16, 32, 64):
        texts = []
        for origin in ("FT", "FO"):
            texts.append((sentinel * 2, "sentinel-pair", origin, 192))
            for name in (
                "advance-735",
                "advance-736",
                "advance-737",
                "advance-735-plus-one",
            ):
                for n in (1, 4):
                    texts.append(
                        (
                            sentinel + chr(by_name[name]["codepoint"]) * n + sentinel,
                            f"{name}-n{n}",
                            origin,
                            192,
                        )
                    )
        add("metrics", h, texts=texts)
    return pages


def zpl(page):
    # Explicit encoding/layout state; reset CI27's byte image mapping.
    mapping = ",".join(f"{n},{n}" for n in range(256))
    out = f"^XA^PW{page['canvas'][0]}^LL{page['canvas'][1]}^CI27,{mapping}^PA0,0,0,0^FPH,0^CVN^CF0,32,0^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
    for p in page["probes"]:
        x, y, _, _ = p["tile"]
        ax, ay = p["anchor"]
        text = "".join(f"_{ord(c):02X}" for c in p["text"])
        out += f"^FT{x+ax},{y+ay}" if p["origin"] == "FT" else f"^FO{x+ax},{y+ay}"
        out += (
            f"^A@{p['orientation']},{p['height']},{p['width']},{OBJECT}^FH^FD{text}^FS"
        )
    return (out + "^XZ").encode()


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=False)
    items = glyphs()
    data = font(items)
    pages = plan(items)
    (directory / "probe.ttf").write_bytes(data)
    for page in pages:
        data = zpl(page)
        page["zpl_sha256"] = hashlib.sha256(data).hexdigest()
        (directory / (page["name"] + ".zpl")).write_bytes(data)
    manifest = dict(
        schema="constructed-font-live-v1",
        object=OBJECT,
        units_per_em=2048,
        font_sha256=hashlib.sha256((directory / "probe.ttf").read_bytes()).hexdigest(),
        glyphs=items,
        pages=pages,
    )
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output", type=Path, help="New directory for the prepared font and requests"
    )
    args = parser.parse_args()
    manifest = prepare(args.output)
    print(
        f"Prepared {len(manifest['pages'])} pages; font SHA-256 {manifest['font_sha256']}"
    )
