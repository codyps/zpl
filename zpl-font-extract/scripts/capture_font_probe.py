#!/usr/bin/env python3
"""Install one constructed RAM font, capture bounded HTTP previews, and remove it.

Requires an already prepared font_probe.py manifest. This executes on the named
development printer. Protocol references: zebra-http-api/src/lib.rs and
zebra-http-api/examples/font_refine/transport.rs; Zebra ~DY, ^A@, ^FT commands:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
"""
import argparse
from datetime import datetime, timezone
import hashlib
import html
from io import BytesIO
import json
import os
from pathlib import Path
import re
import socket
import time
import urllib.parse
import urllib.request

from PIL import Image


def now():
    return datetime.now(timezone.utc).isoformat()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(path, data):
    with path.open("xb") as f:
        f.write(data)


def save_json(path, data):
    save(path, (json.dumps(data, indent=2) + "\n").encode())


def getvars(host, keys):
    result = {}
    with socket.create_connection((host, 9100), timeout=10) as stream:
        stream.settimeout(10)
        for key in keys:
            stream.sendall(f'! U1 getvar "{key}"\r\n'.encode())
            data = b""
            while data.count(b'"') < 2 and len(data) < 4096:
                chunk = stream.recv(4096)
                if not chunk:
                    raise ValueError("Incomplete identity response")
                data += chunk
            match = re.fullmatch(rb'\s*"([^"\r\n]+)"\s*', data)
            if not match:
                raise ValueError("Invalid identity response")
            result[key] = match[1].decode("ascii")
    return result


def identity(host):
    keys = {
        "device.product_name": "model",
        "device.unique_id": "serial",
        "appl.name": "firmware",
    }
    return {keys[k]: v for k, v in getvars(host, keys).items()}


def send(host, data):
    with socket.create_connection((host, 9100), timeout=10) as stream:
        stream.settimeout(10)
        stream.sendall(data)
        stream.shutdown(socket.SHUT_WR)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError("Printer redirect rejected")


class Printer:
    def __init__(self, host):
        self.host = host
        self.base = "http://" + host + "/"
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}), NoRedirect()
        )

    def fetch(self, path, data=None):
        url = urllib.parse.urljoin(self.base, path)
        p = urllib.parse.urlsplit(url)
        if (
            (p.scheme, p.hostname, p.port) != ("http", self.host, None)
            or p.username
            or p.password
        ):
            raise ValueError("Unexpected preview origin")
        with self.opener.open(
            urllib.request.Request(url, data=data), timeout=30
        ) as response:
            body = response.read(16 * 1024 * 1024 + 1)
        if len(body) > 16 * 1024 * 1024:
            raise ValueError("Printer response too large")
        return body

    def directory(self):
        data = self.fetch("dir?dev=R&otype=TTF")
        if b"Directory of" not in data or b"R:" not in data:
            raise ValueError("Invalid RAM font directory")
        return data

    def preview(self, zpl):
        form = urllib.parse.urlencode(
            dict(
                dev="R",
                oname="ZP26PV",
                otype="ZPL",
                username="",
                pw="",
                data=zpl.decode("ascii"),
                prev="Preview Label",
            )
        ).encode()
        page = self.fetch("zpl", form)
        match = re.search(rb'<IMG\s+SRC="([^"]+)"', page, re.I)
        if not match:
            raise ValueError("Preview did not return an image")
        return self.fetch(html.unescape(match[1].decode()))


def capture(root, output, host, model, serial, force_label_length=False):
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest["schema"] != "constructed-font-live-v1":
        raise ValueError("Wrong manifest")
    obj = manifest["object"]
    if not re.fullmatch(r"R:ZP[0-9A-Z]{1,6}\.TTF", obj):
        raise ValueError("Unexpected RAM object")
    font = (root / "probe.ttf").read_bytes()
    if sha(font) != manifest["font_sha256"] or len(font) > 65536:
        raise ValueError("Font hash/size mismatch")
    for page in manifest["pages"]:
        if sha((root / (page["name"] + ".zpl")).read_bytes()) != page["zpl_sha256"]:
            raise ValueError("Request hash mismatch")
    if len(manifest["pages"]) + 4 > 32:
        raise ValueError("Campaign exceeds 32 preview requests")
    output.mkdir(parents=True, exist_ok=False)
    lock = Path("/tmp") / f"zpl-font-probe-{serial}.lock"
    with lock.open("x") as f:
        f.write(str(os.getpid()))
    installed = False
    restore_settings = {}
    record = dict(
        schema="constructed-font-capture-v1",
        started_utc=now(),
        host=host,
        manifest_sha256=sha((root / "manifest.json").read_bytes()),
        pages=[],
        status="running",
    )
    printer = Printer(host)
    try:
        device = identity(host)
        if (device["model"], device["serial"]) != (model, serial):
            raise ValueError("Printer identity mismatch")
        home = printer.fetch("/")
        if b"203dpi" not in home:
            raise ValueError("203 DPI not confirmed by printer")
        record["printer"] = {**device, "dpi": 203}
        print("Verified", host, device, flush=True)
        directory = printer.directory()
        if obj.encode() in directory:
            raise ValueError("Probe object already exists; refusing overwrite")
        if force_label_length:
            # ^LLh,Y applies label length to mark/gap media. Save and restore both
            # affected settings; no media-tracking change or ^JUS is needed.
            # https://docs.zebra.com/content/tcm/us/en/printers/software/zpl-pg/zpl-commands/%5Ell.html
            settings = getvars(host, ["zpl.label_length_always", "zpl.label_length"])
            if settings["zpl.label_length_always"] not in (
                "yes",
                "no",
            ) or not re.fullmatch(r"[0-9]+", settings["zpl.label_length"]):
                raise ValueError("Unsupported label-length settings")
            restore_settings = settings
            record["label_length_settings_before"] = restore_settings

        def preview(name, zpl, dimensions):
            if len(record["pages"]) >= 32:
                raise ValueError("Preview budget exhausted")
            source_hash = sha(zpl)
            if force_label_length:
                zpl, n = re.subn(rb"\^LL([0-9]+)(?=\^)", rb"^LL\1,Y", zpl, count=1)
                if n != 1:
                    raise ValueError("Expected one explicit label length")
            row = dict(
                name=name,
                started_utc=now(),
                prepared_zpl_sha256=source_hash,
                zpl_sha256=sha(zpl),
                status="reserved",
            )
            record["pages"].append(row)
            with (output / "requests.jsonl").open("a") as f:
                f.write(json.dumps(row) + "\n")
                f.flush()
                os.fsync(f.fileno())
            save(output / (name + ".zpl"), zpl)
            time.sleep(5)
            data = printer.preview(zpl)
            save(output / (name + ".png"), data)
            with Image.open(BytesIO(data)) as image:
                row.update(png_sha256=sha(data), dimensions=list(image.size))
                if list(image.size) != dimensions:
                    raise ValueError(
                        f"Native canvas differs: {image.size} vs {dimensions}"
                    )
                gray = image.convert("L")
                colors = gray.getextrema()
                if colors != (0, 255):
                    raise ValueError("Blank or non-binary preview range")
                if any(n not in (0, 255) for n in gray.get_flattened_data()):
                    raise ValueError("Non-binary preview")
            row.update(status="captured", png_sha256=sha(data), dimensions=dimensions)
            save(output / (name + ".sha256"), sha(data).encode())
            print(host, name, "captured", flush=True)
            return data

        control = b"^XA^PW384^LL192^CI27^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN^FT16,48^A0N,32,0^FDHIl|OonmgjpAV_.^FS^FO16,96^GB96,32,2^FS^XZ"
        first = preview("resident-start", control, [384, 192])
        # Persist ownership before the one permitted upload attempt, so cleanup
        # remains possible after a partial TCP write or verification failure.
        save_json(
            output / "installation.json",
            dict(object=obj, font_sha256=sha(font), started_utc=now(), printer=device),
        )
        installed = True
        with socket.create_connection((host, 9100), timeout=10) as stream:
            stream.settimeout(10)
            stream.sendall(f"~DY{obj},B,T,{len(font)},,".encode() + font)
            stream.shutdown(socket.SHUT_WR)
        time.sleep(1)
        if obj.encode() not in printer.directory():
            raise ValueError("Font upload not confirmed")
        print(host, "font installed", len(font), "bytes", flush=True)
        # First state page is both a distinctive identity and hint execution check.
        for page in manifest["pages"]:
            data = preview(
                page["name"],
                (root / (page["name"] + ".zpl")).read_bytes(),
                page["canvas"],
            )
            if page["name"] == "00-state":
                # At 16 ppem the design-unit markers lie on integral pixels.
                # SCFS sets a 13-dot width independently of font scaling.
                with Image.open(BytesIO(data)) as source:
                    image = source.convert("L")
                    for name, rectangles in (
                        ("identity", [(0, -4, 1, 4), (2, -6, 1, 6), (4, -8, 2, 8)]),
                        ("instruction-witness", [(0, -2, 13, 2)]),
                    ):
                        p = next(p for p in page["probes"] if p["name"] == name)
                        tx, ty, tw, th = p["tile"]
                        ax, ay = p["anchor"]
                        actual = {
                            (x, y)
                            for y in range(ty, ty + th)
                            for x in range(tx, tx + tw)
                            if image.getpixel((x, y)) == 0
                        }
                        expected = {
                            (tx + ax + x, ty + ay + y)
                            for rx, ry, rw, rh in rectangles
                            for x in range(rx, rx + rw)
                            for y in range(ry, ry + rh)
                        }
                        if actual != expected:
                            raise ValueError(
                                f"Constructed-font selection/execution control failed: {name}"
                            )
                record["font_selection_and_execution_verified"] = True
        last = preview("resident-end", control, [384, 192])
        with Image.open(BytesIO(first)) as a, Image.open(BytesIO(last)) as b:
            record["resident_repeat_exact"] = (
                a.convert("L").tobytes() == b.convert("L").tobytes()
            )
        if not record["resident_repeat_exact"]:
            raise ValueError("Resident control changed during campaign")
        if identity(host) != device:
            raise ValueError("Printer identity changed during campaign")
        record["status"] = "complete"
    except Exception as error:
        record["status"] = "failed"
        record["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        try:
            if installed:
                # Documented SGD deletion, restricted to this owned RAM name:
                # https://docs.zebra.com/us/en/printers/software/zpl-pg/c-sgd-commands-from-a-to-d/r-sgd-file-delete.html
                send(host, f'! U1 do "file.delete" "{obj}"\r\n'.encode())
                time.sleep(1)
                absent = obj.encode() not in printer.directory()
                record["cleanup"] = dict(
                    object=obj, method="SGD file.delete", confirmed_absent=absent
                )
                print(host, "cleanup confirmed", absent, flush=True)
                if not absent:
                    raise ValueError("Temporary font deletion not confirmed")
        except Exception as error:
            record["cleanup_error"] = f"{type(error).__name__}: {error}"
            record["status"] = "cleanup-failed"
            raise
        finally:
            try:
                if restore_settings:
                    for key, value in restore_settings.items():
                        send(host, f'! U1 setvar "{key}" "{value}"\r\n'.encode())
                    restored = getvars(host, restore_settings)
                    record["label_length_settings_after"] = restored
                    if restored != restore_settings:
                        raise ValueError("Label-length settings restoration failed")
            except Exception as error:
                record["restoration_error"] = f"{type(error).__name__}: {error}"
                record["status"] = "restoration-failed"
                raise
            finally:
                record["finished_utc"] = now()
                save_json(output / "capture.json", record)
                lock.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prepared", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--host", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--serial", required=True)
    parser.add_argument(
        "--force-label-length",
        action="store_true",
        help="Use ^LLh,Y, restoring original settings afterward",
    )
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z0-9.-]+", args.host) or not re.fullmatch(
        r"[a-zA-Z0-9]+", args.serial
    ):
        parser.error("Expected a host name and alphanumeric serial")
    capture(
        args.prepared,
        args.output,
        args.host,
        args.model,
        args.serial,
        args.force_label_length,
    )
