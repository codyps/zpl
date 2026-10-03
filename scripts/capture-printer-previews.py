#!/usr/bin/env python3
"""Inventory and recapture checked-in printer previews, without physical printing.

Protocol: zebra-http-api/src/lib.rs. Reset: sibling comparison capture.py.
See docs/printer-recapture.md for scope and the width-only transformation.
"""
import argparse
import hashlib
import html
import json
from pathlib import Path
import re
import subprocess
import struct
import time
import zlib
import urllib.parse
import urllib.request
from datetime import datetime, timezone

LIMIT = 16 * 1024 * 1024
SCOPES = {
    "zpl": ("zpl/tests/fixtures/", "zebra-http-api/tests/fixtures/",
            "zpl-font-extract/tests/fixtures/"),
    "zpl-comparison": ("benchmarks/accuracy/", "references/"),
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def now():
    return datetime.now(timezone.utc).isoformat()


def png_dimensions(data):
    """Check PNG framing/CRCs and report native IHDR dimensions, without resizing."""
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("not PNG")
    offset, dimensions = 8, None
    while offset + 12 <= len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        end = offset + 8 + length
        if end + 4 > len(data):
            raise ValueError("truncated PNG")
        kind = data[offset + 4:offset + 8]
        if zlib.crc32(data[offset + 4:end]) != struct.unpack_from(">I", data, end)[0]:
            raise ValueError("PNG CRC mismatch")
        if offset == 8:
            if kind != b"IHDR" or length != 13:
                raise ValueError("missing PNG IHDR")
            dimensions = list(struct.unpack_from(">II", data, offset + 8))
        if kind == b"IEND":
            if end + 4 != len(data):
                raise ValueError("trailing PNG bytes")
            return dimensions
        offset = end + 4
    raise ValueError("missing PNG IEND")


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def save(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def verify(directory):
    manifest = json.loads((directory / "provenance.json").read_text())
    if sha((directory / "plan.json").read_bytes()) != manifest["plan_sha256"]:
        raise ValueError("saved plan hash mismatch")
    for path, digest in manifest["files"].items():
        target = (directory / path).resolve()
        if not target.is_relative_to(directory.resolve()):
            raise ValueError("manifest path escapes output")
        if sha(target.read_bytes()) != digest:
            raise ValueError(f"saved file hash mismatch: {path}")
    return manifest


def reset():
    identity = ",".join(f"{n},{n}" for n in range(256))
    return (f"^CI0,{identity}^CI13,{identity}"
            "^PMN^PA0,0,0,0^FPH,0^CVN^BY2,3,10^CI27^CF0,32,0"
            "^FWN^LH0,0^LS0^LT0^PON^LRN").encode()


def adapt(source, width, height):
    """Preserve field bytes; reject unsupported framing instead of corrupting it.

    Only standard caret commands are accepted. GFB uses its declared byte count;
    compressed GFC is explicitly unsupported.
    """
    if b"\0" in source:
        raise ValueError("literal NUL is not supported by the HTTP preview form")
    if not source.lstrip().startswith(b"^XA"):
        raise ValueError("nonstandard format framing")
    allowed = set("XA XZ PW LL FO FT FS FD FX FH FE A@ CF CI PA FP CV BY FW LH LS LT PO LR PM GB GC GD GE GF GS FR FB TB FN FV SN SF FC FM PQ PF MU".split())
    pieces, widths, i, formats = [], [], 0, 0
    while i < len(source):
        if source[i:i+1] != b"^":
            if source[i:i+1].strip():
                raise ValueError("unframed bytes outside a command")
            pieces.append(source[i:i+1])
            i += 1
            continue
        command = source[i+1:i+3].decode("ascii", errors="replace")
        if command not in allowed and not re.fullmatch(r"[AB][A-Z0-9]", command):
            raise ValueError(f"unsupported preview command ^{command}")
        if command in ("FD", "FV", "FX"):
            end = source.find(b"^FS", i + 3)
            if end < 0:
                raise ValueError("unterminated field/comment")
        else:
            end = source.find(b"^", i + 3)
            if end < 0:
                end = len(source)
        chunk = source[i:end]
        if command == "GF" and chunk.startswith(b"^GFB,"):
            header = re.match(rb"\^GFB,(\d+),(\d+),(\d+),", source[i:])
            if not header:
                raise ValueError("invalid binary GF header")
            end = i + header.end() + int(header[1])
            if end > len(source):
                raise ValueError("truncated binary GF")
            chunk = source[i:end]
        elif command == "GF" and not chunk.startswith(b"^GFA,"):
            raise ValueError("compressed GF requires byte-stream parser")
        if command not in ("FD", "FV", "FX", "GF") and b"~" in chunk and not (command == "BX" and chunk.rstrip().endswith(b",~") and chunk.count(b"~") == 1):
            raise ValueError("control prefix outside field")
        if command == "XA":
            formats += 1
            chunk += f"^PW{width}^LL{height}".encode() + reset()
        elif command == "PW":
            match = re.fullmatch(rb"\^PW(\d+)\s*", chunk)
            if not match:
                raise ValueError("non-numeric PW")
            old = int(match[1])
            widths.append(old)
            chunk = f"^PW{min(old, width)}".encode()
        pieces.append(chunk)
        i = end
    if formats != 1 or not source.rstrip().endswith(b"^XZ"):
        raise ValueError("expected one complete label")
    return b"".join(pieces), widths


def inventory(roots, width):
    result = {"schema": 1, "width_dots": width,
              "policy": "clamp PW; preserve geometry; inline identity-map/layout reset, BY height 10",
              "repositories": {}, "cases": [], "unresolved": [], "excluded": []}
    for name, root in roots.items():
        files = git(root, "ls-files", "-z").decode().split("\0")[:-1]
        result["repositories"][name] = {
            "commit": git(root, "rev-parse", "HEAD").decode().strip(),
            "status": git(root, "status", "--porcelain").decode(),
        }
        sources = {}
        for path in files:
            if path.endswith(".zpl"):
                sources.setdefault(Path(path).stem, []).append(path)
        for path in sorted(files):
            if not path.endswith(".png") or not path.startswith(SCOPES[name]):
                continue
            if "labelary" in path:
                result["excluded"].append({"path": f"{name}/{path}", "reason": "Labelary, not printer preview"})
                continue
            png = (root / path).read_bytes()
            sibling = str(Path(path).with_suffix(".zpl"))
            candidates = [sibling] if sibling in files else sources.get(Path(path).stem, [])
            audit = None
            if "/height-only-captures/" in path:
                suite, case = Path(path).stem.split("--", 1)
                report = Path(path).parent.parent / f"height-only-{suite}.json"
                audit = json.loads((root / report).read_text())
                candidates = [str(Path(audit["saved_reference"]) / (case + ".zpl"))]
                entry = next(r for r in audit["cases"] if r["name"] == case)
                if sha(png) != entry["recaptured_png_sha256"] or sha((root / candidates[0]).read_bytes()) != entry["zpl_sha256"]:
                    raise ValueError("historical audit hash mismatch")
            unique = {sha((root / p).read_bytes()): p for p in candidates}
            if len(unique) != 1:
                result["unresolved"].append({"path": f"{name}/{path}", "png_sha256": sha(png),
                                             "reason": "missing or ambiguous source", "candidates": candidates})
                continue
            source_path = next(iter(unique.values()))
            source = (root / source_path).read_bytes()
            dimensions = png_dimensions(png)
            row = {"id": f"{name}/{path[:-4]}", "source": f"{name}/{source_path}",
                   "source_sha256": sha(source), "reference": f"{name}/{path}",
                   "reference_sha256": sha(png), "reference_dimensions": dimensions}
            try:
                submitted, widths = adapt(source, width, dimensions[1])
                row.update(submitted_sha256=sha(submitted), original_widths=widths, status="pending")
            except ValueError as error:
                row.update(status="unsupported", error=str(error))
                if source.startswith(b"~DY"):
                    row["error"] = "requires font download/stored-object mutation; excluded from preview-only replay"
            result["cases"].append(row)
    return result


class SameOrigin(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl)[:2] != urllib.parse.urlsplit(req.full_url)[:2]:
            raise ValueError("cross-origin redirect")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


class Printer:
    def __init__(self, host):
        url = urllib.parse.urlsplit(host)
        if url.scheme not in ("http", "https") or not url.hostname or url.username or url.password or url.query or url.fragment or url.path not in ("", "/"):
            raise ValueError("host must be a credential-free HTTP(S) origin")
        self.base = host.rstrip("/") + "/"
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), SameOrigin())

    def fetch(self, path, data=None):
        url = urllib.parse.urljoin(self.base, path)
        parsed = urllib.parse.urlsplit(url)
        if parsed[:2] != urllib.parse.urlsplit(self.base)[:2] or parsed.username or parsed.password:
            raise ValueError("invalid preview origin")
        with self.opener.open(urllib.request.Request(url, data=data), timeout=30) as response:
            body = response.read(LIMIT + 1)
            if len(body) > LIMIT:
                raise ValueError("response exceeds 16 MiB")
            return body

    def preview(self, submitted):
        form = urllib.parse.urlencode({"dev": "R", "oname": "ZQCAP", "otype": "ZPL",
                                      "username": "", "pw": "", "data": submitted,
                                      "prev": "Preview Label"}).encode()
        page = self.fetch("zpl", form)
        match = re.search(rb'<IMG\s+SRC="([^"]+)"', page, re.I)
        if not match:
            raise ValueError("preview response contains no image")
        image_url = html.unescape(match[1].decode())
        return page, image_url, self.fetch(image_url)


def identity(page):
    text = html.unescape(page.decode("latin1"))
    model = re.search(r"ZTC [^<\r\n]+", text)
    firmware = re.search(r"Firmware Version</TD>\s*<TD>([^<]+)", text, re.I)
    serial = re.search(r"BT Friendly Name</TD><TD>([^<]+)", text, re.I)
    if not all((model, firmware, serial)):
        raise ValueError("cannot identify model, firmware and serial from status page")
    return {"model": model[0], "firmware": firmware[1], "serial": serial[1], "dpi": 203}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--zpl", type=Path, default=Path(__file__).resolve().parents[1])
    ap.add_argument("--comparison", type=Path)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--width", type=int, default=384)
    ap.add_argument("--host", help="printer origin; required with --capture")
    ap.add_argument("--capture", action="store_true", help="perform previews; otherwise inventory only")
    ap.add_argument("--verify", action="store_true", help="verify saved hashes offline")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--limit", type=int, help="capture a bounded smoke-test subset")
    ap.add_argument("--interval", type=float, default=2)
    args = ap.parse_args()
    if args.capture and not args.host:
        ap.error("--host is required with --capture")
    if args.verify:
        manifest = verify(args.output)
        print(f"Verified {len(manifest['files'])} files; status: {manifest['status']}")
        return
    if not 1 <= args.width <= 384 or args.interval < 0 or (args.limit is not None and args.limit < 1):
        ap.error("width must be 1..384, interval nonnegative, limit positive")
    roots = {"zpl": args.zpl.resolve(), "zpl-comparison": (args.comparison or args.zpl.parent / "zpl-comparison").resolve()}
    plan = inventory(roots, args.width)
    args.output.mkdir(parents=True, exist_ok=args.resume)
    plan_file = args.output / "plan.json"
    if args.resume:
        previous = json.loads(plan_file.read_text())
        # Dirty status naturally changes during development; fixture hashes and
        # commits, rather than unrelated edits, determine plan compatibility.
        for repo in plan["repositories"]:
            previous["repositories"][repo]["status"] = plan["repositories"][repo]["status"]
        if previous != plan:
            raise ValueError("source inventory changed; use a new output directory")
    else:
        save(plan_file, plan)
    counts = {s: sum(r["status"] == s for r in plan["cases"]) for s in ("pending", "unsupported")}
    print(json.dumps({**counts, "unresolved": len(plan["unresolved"]), "excluded": len(plan["excluded"])}), flush=True)
    if not args.capture:
        return
    printer = Printer(args.host)
    home = printer.fetch("/")
    device = identity(home)
    if "ZQ610 Plus-203dpi" not in device["model"]:
        raise ValueError("expected ZQ610 Plus 203dpi")
    manifest_file = args.output / "provenance.json"
    if args.resume and manifest_file.exists():
        manifest = verify(args.output)
        if manifest["printer"] != device or manifest["host"] != printer.base:
            raise ValueError("printer identity/firmware/origin changed")
    else:
        manifest = {"schema": 1, "started_utc": now(), "printer": device, "host": printer.base,
                    "method": "HTTP Preview Label; no physical print", "status": "incomplete",
                    "plan_sha256": sha(plan_file.read_bytes()), "files": {}, "cases": {}, "sessions": []}
    def store(path, data):
        target = args.output / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        manifest["files"][path] = sha(data)
    session = {"started_utc": now(), "tool_sha256": sha(Path(__file__).read_bytes())}
    manifest["sessions"].append(session)
    store(f"status-{len(manifest['sessions'])}.html", home)
    store("reset.zpl", reset())
    save(manifest_file, manifest)
    control, _ = adapt(b"^XA^FO20,20^A0N,24,24^FDZQ610 capture control^FS^XZ", args.width, 200)
    def capture_control(label):
        prefix = f"controls/session-{len(manifest['sessions'])}-{label}"
        session[label + "_started_utc"] = now()
        store(prefix + ".submitted.zpl", control)
        save(manifest_file, manifest)
        page, url, png = printer.preview(control)
        store(prefix + ".html", page)
        store(prefix + ".png", png)
        session[label] = {"dimensions": png_dimensions(png), "image_url": url,
                          "png_sha256": sha(png), "finished_utc": now()}
        save(manifest_file, manifest)
        return png
    first_control = capture_control("start")
    captured = 0
    for row in plan["cases"]:
        key = row["id"]
        if row["status"] != "pending" or key in manifest["cases"]:
            continue
        if args.limit is not None and captured >= args.limit:
            break
        repo, relative = row["source"].split("/", 1)
        original = (roots[repo] / relative).read_bytes()
        submitted, _ = adapt(original, args.width, row["reference_dimensions"][1])
        if sha(submitted) != row["submitted_sha256"]:
            raise ValueError("source changed after inventory")
        prefix = "captures/" + key
        record = {"started_utc": now(), "status": "inflight", "submitted_sha256": sha(submitted)}
        manifest["cases"][key] = record
        store(prefix + ".source.zpl", original)
        store(prefix + ".submitted.zpl", submitted)
        save(manifest_file, manifest)
        try:
            time.sleep(args.interval)
            page, url, png = printer.preview(submitted)
            store(prefix + ".html", page)
            store(prefix + ".png", png)
            record["dimensions"] = png_dimensions(png)
            record["canvas_matches_requested"] = record["dimensions"] == [min(row["original_widths"][-1], args.width) if row["original_widths"] else args.width, row["reference_dimensions"][1]]
            record.update(status="captured", image_url=url)
        except Exception as error:
            record.update(status="failed", error=str(error), finished_utc=now())
            save(manifest_file, manifest)
            raise  # Never automatically replay a possibly executing POST.
        record["finished_utc"] = now()
        captured += 1
        save(manifest_file, manifest)
        print(key, record["dimensions"], flush=True)
    remaining = sum(r["status"] == "pending" and manifest["cases"].get(r["id"], {}).get("status") != "captured" for r in plan["cases"])
    time.sleep(args.interval)
    session["repeat_png_bytes_equal"] = first_control == capture_control("end")
    manifest["status"] = "complete" if not remaining and not counts["unsupported"] and not plan["unresolved"] and all(s.get("repeat_png_bytes_equal") for s in manifest["sessions"]) else "incomplete"
    session["finished_utc"] = now()
    manifest["updated_utc"] = now()
    save(manifest_file, manifest)


if __name__ == "__main__":
    main()
