"""Validate the generated Font 0 hint model through the printer's TTF path.

Temporary RAM object selection and state witnesses follow capture_font_probe.py.
The model must be frozen before preparing this campaign.
"""

import argparse
import json
from pathlib import Path

from capture_font_probe import save, save_json, sha
import font0_hint_model as hints


def prepare(root, source, development, validation, model_path):
    root.mkdir(parents=True, exist_ok=False)
    model = json.loads(model_path.read_text())
    assert model["source_manifest_sha256"] == sha(
        (source / "manifest.json").read_bytes()
    )
    assert model["development_manifest_sha256"] == sha(
        (development / "manifest.json").read_bytes()
    )
    data = hints.compile_hints(
        hints.load_models(source), model["hints"], model["parameters"], witnesses=True
    )
    save(root / "probe.ttf", data)
    save(root / "model.json", model_path.read_bytes())
    pages = []
    state = [
        dict(
            name=name,
            text=chr(33 + i),
            tile=[i * 64, 0, 64, 64],
            anchor=[16, 32],
            width=16,
            height=16,
            orientation="N",
            origin="FT",
        )
        for i, name in enumerate(["identity", "instruction-witness"])
    ]
    requests = [dict(name="00-state", canvas=[384, 64], probes=state, group="state")]
    for directory, group in [(development, "development"), (validation, "validation")]:
        manifest = json.loads((directory / "manifest.json").read_text())
        requests.extend(
            p for p in manifest["pages"] if p["font"] == "0" and p["group"] == group
        )
    for page in requests:
        width, height = page["canvas"]
        zpl = f"^XA^PW{width}^LL{height}^CI28^PA0,0,0,0^FPH,0^CVN^FWN^LH0,0^LS0^LT0^PON^LRN^PMN"
        for p in page["probes"]:
            tx, ty, _, _ = p["tile"]
            ax, ay = p["anchor"]
            text = "".join(f"_{b:02X}" for b in p["text"].encode())
            zpl += f"^FT{tx+ax},{ty+ay}^A@{p['orientation']},{p['height']},{p['width']},R:ZP26F.TTF^FH_^FD{text}^FS"
        request = (zpl + "^XZ").encode()
        save(root / (page["name"] + ".zpl"), request)
        pages.append(
            dict(
                name=page["name"],
                group=page["group"],
                canvas=page["canvas"],
                probes=page["probes"],
                zpl_sha256=sha(request),
            )
        )
    save_json(
        root / "manifest.json",
        dict(
            schema="constructed-font-live-v1",
            object="R:ZP26F.TTF",
            font_sha256=sha(data),
            model_sha256=sha(model_path.read_bytes()),
            source_manifest_sha256=model["source_manifest_sha256"],
            development_manifest_sha256=model["development_manifest_sha256"],
            validation_manifest_sha256=sha((validation / "manifest.json").read_bytes()),
            pages=pages,
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["root", "source", "development", "validation", "model"]:
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    prepare(args.root, args.source, args.development, args.validation, args.model)
