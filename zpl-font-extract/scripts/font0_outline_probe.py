"""Large native Font 0 silhouettes for a bounded outline-recovery pilot.

Zebra ^A0 / ^FT: ZPL Programming Guide command reference.
https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
No original font bytes are retrieved or claimed recovered. Sizes 320 and 448
are reserved for subsequent fitting validation, not fitting inputs.
"""

import argparse
from io import BytesIO
import json
import os
from pathlib import Path
import time

from PIL import Image
from capture_font_probe import Printer, identity, now, save, save_json, sha
from swiss_probe import CONTROL


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    pages = []
    for size in (256, 384, 512, 320, 448):
        for text in ("HO", "Sg", "j@"):
            probes = []
            zpl = "^XA^PW768^LL1408^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
            for i, ch in enumerate(text):
                anchor = [64, i * 704 + 544]
                probes.append(
                    dict(
                        text=ch,
                        width=0,
                        height=size,
                        orientation="N",
                        origin="FT",
                        anchor=anchor,
                        tile=[0, i * 704, 768, 704],
                    )
                )
                zpl += (
                    f"^FT{anchor[0]},{anchor[1]}^A0N,{size},0^FH_^FD_{ord(ch):02X}^FS"
                )
            name = f"outline-{size}-{text.replace('@','at')}"
            data = (zpl + "^XZ").encode()
            save(root / (name + ".zpl"), data)
            pages.append(
                dict(
                    name=name,
                    group="development" if size in (256, 384, 512) else "validation",
                    canvas=[768, 1408],
                    probes=probes,
                    zpl_sha256=sha(data),
                )
            )
    save_json(
        root / "manifest.json",
        dict(schema="font0-outline-pilot-v1", font="resident ^A0", pages=pages),
    )


def capture(root, host):
    manifest = json.loads((root / "manifest.json").read_text())
    assert manifest["schema"] == "font0-outline-pilot-v1"
    output = root / "zd621"
    output.mkdir(exist_ok=False)
    lock = Path("/tmp/zpl-font-probe-D7J211001302.lock")
    with lock.open("x") as f:
        f.write(str(os.getpid()))
    record = dict(
        schema="font0-outline-capture-v1",
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

        def preview(name, data, dimensions):
            row = dict(
                name=name, zpl_sha256=sha(data), started_utc=now(), status="requested"
            )
            record["pages"].append(row)
            save(output / (name + ".zpl"), data)
            time.sleep(5)
            png = printer.preview(data)
            save(output / (name + ".png"), png)
            with Image.open(BytesIO(png)) as im:
                assert list(im.size) == dimensions
                pixels = im.convert("L").tobytes()
                assert set(pixels) == {0, 255}
            row.update(status="captured", png_sha256=sha(png), dimensions=dimensions)
            print(name, "captured", flush=True)
            return pixels

        start = preview("resident-start", CONTROL, [384, 192])
        first = None
        for page in manifest["pages"]:
            data = (root / (page["name"] + ".zpl")).read_bytes()
            assert sha(data) == page["zpl_sha256"]
            pixels = preview(page["name"], data, page["canvas"])
            if first is None:
                first = pixels
        page = manifest["pages"][0]
        repeated = preview(
            "outline-repeat",
            (root / (page["name"] + ".zpl")).read_bytes(),
            page["canvas"],
        )
        record["outline_repeat_exact"] = first == repeated
        record["resident_repeat_exact"] = start == preview(
            "resident-end", CONTROL, [384, 192]
        )
        assert record["outline_repeat_exact"] and record["resident_repeat_exact"]
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
