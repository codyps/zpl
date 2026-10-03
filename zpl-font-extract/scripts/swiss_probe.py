"""Native-origin Swiss-font development and reserved validation previews.

Uses the already installed E:TT0003M_.TTF; no font upload or physical printing.
ZPL ^A@ / ^FT / ^CI / ^FH: Zebra Programming Guide, command reference:
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
"""

import argparse
import ftplib
from io import BytesIO
import json
import math
import os
from pathlib import Path
import time

from PIL import Image

from capture_font_probe import Printer, identity, now, save, save_json, sha

FONT_SHA = "da20a4d8c58b3ed09fb9177e09378bd1b824894751add6c1267c64d58f026456"
FONT = "E:TT0003M_.TTF"
CONTROL = b"^XA^PW384^LL192^CI27^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN^FT16,48^A0N,32,0^FDHIl|OonmgjpAV_.^FS^FO16,96^GB96,32,2^FS^XZ"


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    pages = []

    def page(name, x, y, orientation, texts, group, tile=64, wide=False):
        tw = 192 if wide else tile
        cols = 384 // tw
        height = math.ceil(len(texts) / cols) * tile
        assert height <= 1024
        probes = []
        zpl = f"^XA^PW384^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for i, text in enumerate(texts):
            tx, ty = i % cols * tw, i // cols * tile
            ax, ay = 16, 16 + y
            ax, ay = {
                "N": (ax, ay),
                "R": (tile - ay, ax),
                "I": (tile - ax, tile - ay),
                "B": (ay, tile - ax),
            }[orientation]
            probes.append(
                dict(
                    text=text,
                    width=x,
                    height=y,
                    orientation=orientation,
                    origin="FT",
                    tile=[tx, ty, tw, tile],
                    anchor=[ax, ay],
                )
            )
            data = "".join(f"_{b:02X}" for b in text.encode("utf-8"))
            zpl += f"^FT{tx+ax},{ty+ay}^A@{orientation},{y},{x},{FONT}^FH_^FD{data}^FS"
        data = (zpl + "^XZ").encode("ascii")
        save(root / (name + ".zpl"), data)
        pages.append(
            dict(
                name=name,
                group=group,
                canvas=[384, height],
                probes=probes,
                zpl_sha256=sha(data),
            )
        )

    ascii_text = list(map(chr, range(32, 127)))
    for x, y, r in [
        (16, 16, "N"),
        (32, 32, "N"),
        (19, 32, "N"),
        (32, 19, "N"),
        (32, 32, "R"),
        (32, 32, "I"),
        (32, 32, "B"),
    ]:
        page(f"ascii-{x}-{y}-{r}", x, y, r, ascii_text, "development")
    page(
        "ascii-64-64-N",
        64,
        64,
        "N",
        list("AHIOWZabefgijlmnorstuvy0123456789"),
        "development",
        tile=96,
    )
    page(
        "composites-32",
        32,
        32,
        "N",
        list("AÀÁÂÃÄÅaàáâãäåEÈÉÊËeèéêëNÑnñCÇcçÖöÜüÝýÿß"),
        "development",
    )
    for x, y in [(16, 16), (32, 32), (19, 32)]:
        page(
            f"spacing-{x}-{y}",
            x,
            y,
            "N",
            [c * 4 + "|" for c in "HIlm0W ."],
            "development",
            wide=True,
        )
    # Selected before inspecting pixels; compare only after freezing a candidate.
    for x, y, r in [(37, 23, "N"), (23, 37, "N"), (23, 37, "R")]:
        page(f"validation-{x}-{y}-{r}", x, y, r, ascii_text, "validation")
    save_json(
        root / "manifest.json",
        dict(
            schema="swiss-font-probe-v1",
            font=FONT,
            font_sha256=FONT_SHA,
            coordinate_policy="native full canvases, explicit FT origins, no alignment or rescaling",
            pages=pages,
        ),
    )


def capture(root, host):
    manifest = json.loads((root / "manifest.json").read_text())
    assert (
        manifest["schema"] == "swiss-font-probe-v1"
        and manifest["font_sha256"] == FONT_SHA
    )
    output = root / "zd621"
    output.mkdir(exist_ok=False)
    lock = Path("/tmp/zpl-font-probe-D7J211001302.lock")
    with lock.open("x") as f:
        f.write(str(os.getpid()))
    record = dict(
        schema="swiss-font-capture-v1",
        started_utc=now(),
        host=host,
        manifest_sha256=sha((root / "manifest.json").read_bytes()),
        pages=[],
        status="running",
    )
    try:
        device = identity(host)
        assert (device["model"], device["serial"], device["firmware"]) == (
            "ZD621",
            "D7J211001302",
            "V93.21.33Z",
        )
        printer = Printer(host)
        assert b"203dpi" in printer.fetch("/")
        record["printer"] = dict(**device, dpi=203)
        with ftplib.FTP(host, timeout=10) as ftp:
            ftp.login()
            data = bytearray()
            ftp.retrbinary("RETR TT0003M_.TTF.", data.extend)
        assert sha(data) == FONT_SHA, "Installed font differs"
        record["installed_font_sha256"] = sha(data)
        print("Verified", device, "font", sha(data), flush=True)

        def preview(name, data, dimensions):
            row = dict(
                name=name, started_utc=now(), zpl_sha256=sha(data), status="requested"
            )
            record["pages"].append(row)
            save(output / (name + ".zpl"), data)
            time.sleep(5)
            png = printer.preview(data)
            save(output / (name + ".png"), png)
            with Image.open(BytesIO(png)) as im:
                assert list(im.size) == dimensions, (im.size, dimensions)
                pixels = im.convert("L").tobytes()
                assert set(pixels) == {0, 255}, "Blank or nonbinary preview"
            row.update(status="captured", png_sha256=sha(png), dimensions=dimensions)
            print(name, "captured", flush=True)
            return pixels

        start = preview("resident-start", CONTROL, [384, 192])
        repeated = None
        for page in manifest["pages"]:
            data = (root / (page["name"] + ".zpl")).read_bytes()
            assert sha(data) == page["zpl_sha256"]
            pixels = preview(page["name"], data, page["canvas"])
            if page["name"] == "ascii-32-32-N":
                repeated = pixels
        repeat = preview(
            "swiss-repeat", (root / "ascii-32-32-N.zpl").read_bytes(), [384, 1024]
        )
        record["swiss_repeat_exact"] = repeated == repeat
        end = preview("resident-end", CONTROL, [384, 192])
        record["resident_repeat_exact"] = start == end
        assert record["swiss_repeat_exact"] and record["resident_repeat_exact"]
        assert identity(host) == device
        record["status"] = "complete"
    except Exception as error:
        record.update(status="failed", error=str(error))
        raise
    finally:
        record["finished_utc"] = now()
        save_json(output / "capture.json", record)
        lock.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "capture"])
    parser.add_argument("root", type=Path)
    parser.add_argument("--host", default="d7j211001302.bed.einic.org")
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(args.root)
    else:
        capture(args.root, args.host)
