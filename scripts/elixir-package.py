#!/usr/bin/env python3
"""Stage a self-contained Hex source package, without publishing it.

Hex's file list is relative to the Mix project and cannot include sibling crates:
https://hexdocs.pm/hex/Mix.Tasks.Hex.Build.html
Copy only runtime sources and metadata, relocate the path dependency, and prune
unrelated packages from a copy of the workspace lockfile with Cargo metadata.
"""

import argparse
from pathlib import Path
import shutil
import subprocess


def stage(destination: Path) -> None:
    root = Path(__file__).resolve().parents[1]
    binding = root / "zpl-elixir"
    destination.mkdir(parents=True, exist_ok=False)
    for name in ("lib", "src", "test", "examples"):
        shutil.copytree(binding / name, destination / name)
    for name in ("mix.exs", "mix.lock", ".formatter.exs", "README.md", "LICENSE"):
        shutil.copyfile(binding / name, destination / name)

    crates = ("zpl", "zpl-bitmap-fonts", "raster-diff")
    for crate in crates:
        source = root / crate
        target = destination / "native" / crate
        shutil.copytree(source / "src", target / "src")
        for name in ("LICENSE", "README.md", "CHANGELOG.md"):
            if (source / name).exists():
                shutil.copyfile(source / name, target / name)
        # These dependencies are used for Rust repository tests only; carrying
        # them into a runtime source package pulls unrelated decoder/HTTP stacks.
        manifest = (source / "Cargo.toml").read_text()
        if "[dev-dependencies]" in manifest:
            before, after = manifest.split("[dev-dependencies]", 1)
            # Preserve any subsequent tables if one is added later.
            following = after.find("\n[")
            manifest = before + (after[following:] if following >= 0 else "")
        (target / "Cargo.toml").write_text(manifest)

    manifest = (binding / "Cargo.toml").read_text()
    old_path = 'path = "../zpl"'
    if manifest.count(old_path) != 1:
        raise RuntimeError("binding path dependency changed; update the staging script")
    manifest = manifest.replace(old_path, 'path = "native/zpl"')
    manifest += """
[workspace]
resolver = "2"
members = ["native/zpl", "native/zpl-bitmap-fonts", "native/raster-diff"]

[workspace.package]
license = "OSL-3.0"
"""
    (destination / "Cargo.toml").write_text(manifest)
    shutil.copyfile(root / "Cargo.lock", destination / "Cargo.lock")
    subprocess.run(
        ["cargo", "metadata", "--offline", "--format-version=1"],
        cwd=destination,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    print(f"Staged {destination}. Run mix deps.get and mix hex.build there.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path, help="new staging directory")
    stage(parser.parse_args().destination.resolve())
