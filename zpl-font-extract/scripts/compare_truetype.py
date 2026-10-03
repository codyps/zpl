"""Compare original-engine outlines with a separate FreeType implementation.

No printer access. External font bytes and outlines are not saved in reports.
Reference generation is restricted to the repository's original probe font.
FreeType flags: https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html
Run with freetype-py==2.5.1 (the observed library is FreeType 2.13.2).
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess

import freetype

PROBE_SHA = "6366abca64676b4b950cb24fa607c5b29ff5714daf46f3276f1c522ffca07dd9"
FLAGS = (
    freetype.FT_LOAD_TARGET_MONO
    | freetype.FT_LOAD_NO_AUTOHINT
    | freetype.FT_LOAD_PEDANTIC
)


def reference(font, x, y, codepoint):
    face = freetype.Face(str(font))
    face.set_pixel_sizes(x, y)
    face.load_char(chr(codepoint), FLAGS)
    points, tags = face.glyph.outline.points, face.glyph.outline.tags
    contours, start = [], 0
    for end in face.glyph.outline.contours:
        contours.append([[*points[i], tags[i] & 1] for i in range(start, end + 1)])
        start = end + 1
    return dict(
        x=x, y=y, codepoint=codepoint, advance=face.glyph.advance.x, contours=contours
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("font", type=Path)
    parser.add_argument("--engine", type=Path)
    parser.add_argument(
        "--all", action="store_true", help="all nonzero Unicode cmap entries"
    )
    parser.add_argument("--report", type=Path)
    parser.add_argument("--reference-output", type=Path)
    args = parser.parse_args()
    digest = hashlib.sha256(args.font.read_bytes()).hexdigest()
    face = freetype.Face(str(args.font))
    chars = [
        c
        for c, g in face.get_chars()
        if g and c <= 0x10FFFF and (args.all or 32 <= c < 127)
    ]
    metadata = dict(
        font_sha256=digest, freetype_version=list(freetype.version()), load_flags=FLAGS
    )
    if args.reference_output:
        if digest != PROBE_SHA:
            parser.error("reference output requires the original repository probe font")
        sizes = [
            (9, 9),
            (16, 16),
            (31, 31),
            (32, 32),
            (33, 33),
            (64, 64),
            (19, 32),
            (64, 32),
        ]
        cases = [reference(args.font, x, y, c) for x, y in sizes for c in chars]
        args.reference_output.write_text(
            json.dumps(dict(**metadata, cases=cases), separators=(",", ":")) + "\n"
        )
        return
    if not args.engine:
        parser.error("supply --engine path/to/ttf-examine")
    sizes = [(16, 16), (32, 32), (24, 40), (64, 64)]
    queries = [(x, y, c) for x, y in sizes for c in chars]
    result = subprocess.run(
        [str(args.engine.resolve()), "glyphs", str(args.font)],
        input="".join(f"{x} {y} {c} 0\n" for x, y, c in queries),
        text=True,
        capture_output=True,
        check=True,
        timeout=180,
    )
    rows = [json.loads(s) for s in result.stdout.splitlines()]
    counts, distances, failures = Counter(), Counter(), []
    for query, row in zip(queries, rows, strict=True):
        counts["cases"] += 1
        if "error" in row:
            counts["errors"] += 1
            failures.append(dict(query=query, error=row["error"]))
            continue
        expected = reference(args.font, *query)
        counts["exact_outlines"] += row["contours"] == expected["contours"]
        counts["exact_advances"] += row["advance"] == expected["advance"]
        if [len(c) for c in row["contours"]] != [len(c) for c in expected["contours"]]:
            distances["different_topology"] += 1
        else:
            distance = max(
                (
                    abs(a - b)
                    for c, d in zip(row["contours"], expected["contours"], strict=True)
                    for p, q in zip(c, d, strict=True)
                    for a, b in zip(p[:2], q[:2], strict=True)
                ),
                default=0,
            )
            distances[str(distance)] += 1
    report = dict(
        **metadata,
        sizes=sizes,
        all_cmap=args.all,
        counts=dict(counts),
        max_coordinate_error_26_6_histogram=dict(distances),
        failures=failures,
    )
    text = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()
