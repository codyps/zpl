#!/usr/bin/env python3
"""Build a real Mix consumer/release, optionally downloading a NIF over loopback.

Prebuilt mode denies Cargo/rustc and uses an empty cache, proving both the download
and runtime load paths without publishing anything or relying on local NIFs.
"""

import argparse
import functools
import http.server
import json
import os
import shutil
import subprocess
import tempfile
import threading
from pathlib import Path

import tomllib

SMOKE = """
# Release `eval` does not start applications on all supported Elixir versions.
# https://hexdocs.pm/mix/1.15.8/Mix.Tasks.Release.html#module-one-off-commands-eval-and-rpc
{:ok, _} = Application.ensure_all_started(:zpl)
true = Zpl.library_version() == System.fetch_env!("ZPL_EXPECTED_LIBRARY_VERSION")
true = to_string(Application.spec(:zpl, :vsn)) == System.fetch_env!("ZPL_EXPECTED_PACKAGE_VERSION")
source = "^XA^FO2,3^GB4,2,2^FS^XZ"
^source = source |> Zpl.parse!() |> Map.fetch!(:elements) |> Enum.map(& &1.data) |> IO.iodata_to_binary()
document = Zpl.render!(source, profile: :specification, width: 16, height: 12)
[scene] = document.labels
%{width: 16, height: 12, dpi: 203} = scene
expected = for y <- 0..11, x <- 0..15, into: <<>>, do: if(x in 2..5 and y in 3..4, do: <<0>>, else: <<255>>)
^expected = Zpl.Scene.rasterize!(scene).pixels
<<137, "PNG\\r\\n", 26, "\\n", _::binary>> = Zpl.Scene.encode!(scene, :png)
true = String.contains?(Zpl.Scene.encode!(scene, :svg), "<svg")
<<"%PDF-", _::binary>> = Zpl.Document.pdf!(document)
IO.puts("Consumer passed: #{Zpl.library_version()}, parse/raster/PNG/SVG/PDF")
"""


def check(package, artifacts=None, force_build=False, corrupt=False):
    if corrupt and (artifacts is None or force_build):
        raise ValueError("Corruption check requires prebuilt mode")
    with tempfile.TemporaryDirectory(prefix="zpl-consumer-") as tmp:
        root = Path(tmp)
        (root / "mix.exs").write_text(
            """
defmodule Consumer.MixProject do
  use Mix.Project
  def project, do: [app: :consumer, version: "0.1.0", deps: [{:zpl, path: __PACKAGE__}]]
  def application, do: [extra_applications: [:logger]]
end
""".replace("__PACKAGE__", json.dumps(str(package.resolve())))
        )
        (root / "lib").mkdir()
        (root / "lib/consumer.ex").write_text("defmodule Consumer do\nend\n")
        (root / "smoke.exs").write_text(SMOKE)
        env = os.environ.copy()
        env["MIX_ENV"] = "prod"
        renderer = package / "native/zpl/Cargo.toml"
        if not renderer.exists():
            renderer = package.parent / "zpl/Cargo.toml"
        env["ZPL_EXPECTED_LIBRARY_VERSION"] = tomllib.loads(renderer.read_text())[
            "package"
        ]["version"]
        env["ZPL_EXPECTED_PACKAGE_VERSION"] = tomllib.loads(
            (package / "Cargo.toml").read_text()
        )["package"]["version"]
        # Do not inherit a caller's build mode/cache for the prebuilt check.
        env.pop("RUSTLER_PRECOMPILED_FORCE_BUILD_ALL", None)
        env["ZPL_BUILD"] = "true" if force_build else "false"
        env["RUSTLER_PRECOMPILED_GLOBAL_CACHE_PATH"] = str(root / "cache")
        (root / "cache").mkdir()
        server = None
        if artifacts is not None:
            if corrupt:
                shutil.copytree(artifacts, root / "corrupt")
                artifacts = root / "corrupt"
                for path in artifacts.glob("*.tar.gz"):
                    with path.open("ab") as output:
                        output.write(b"corrupt")
            handler = functools.partial(
                http.server.SimpleHTTPRequestHandler, directory=str(artifacts.resolve())
            )
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            (root / "config").mkdir()
            (root / "config/config.exs").write_text(
                f'import Config\nconfig :zpl, :precompiled_base_url, "http://127.0.0.1:{server.server_port}/"\n'
            )
            env["NO_PROXY"] = env.get("NO_PROXY", "") + ",127.0.0.1,localhost"
            env["no_proxy"] = env["NO_PROXY"]
            if not force_build:
                blocked = root / "blocked"
                blocked.mkdir()
                for executable in ("cargo", "rustc"):
                    script = blocked / (
                        executable + (".cmd" if os.name == "nt" else "")
                    )
                    script.write_text(
                        "@echo off\necho Rust must not run in prebuilt mode >&2\nexit /b 97\n"
                        if os.name == "nt"
                        else "#!/bin/sh\necho 'Rust must not run in prebuilt mode' >&2\nexit 97\n"
                    )
                    script.chmod(0o755)
                env["PATH"] = str(blocked) + os.pathsep + env["PATH"]
        try:
            mix = "mix.bat" if os.name == "nt" else "mix"
            if corrupt:
                subprocess.run([mix, "deps.get"], cwd=root, env=env, check=True)
                result = subprocess.run(
                    [mix, "compile"],
                    cwd=root,
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if (
                    result.returncode == 0
                    or "checksum of files does not match"
                    not in result.stdout + result.stderr
                ):
                    raise RuntimeError(
                        f"Expected checksum rejection: {result.stdout}\n{result.stderr}"
                    )
                print("Consumer rejected corrupt prebuilt checksum")
                return
            for args in (
                ["deps.get"],
                ["compile", "--warnings-as-errors"],
                ["run", "smoke.exs"],
                ["release"],
            ):
                subprocess.run([mix, *args], cwd=root, env=env, check=True)
            release = (
                root
                / "_build/prod/rel/consumer/bin"
                / ("consumer.bat" if os.name == "nt" else "consumer")
            )
            subprocess.run(
                [str(release), "eval", "Code.eval_file(~s[smoke.exs])"],
                cwd=root,
                env=env,
                check=True,
            )
            if (
                artifacts is not None
                and not force_build
                and not list((root / "cache").glob("*.tar.gz"))
            ):
                raise RuntimeError("Prebuilt mode did not download a NIF")
        finally:
            if server is not None:
                server.shutdown()
                server.server_close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--force-build", action="store_true")
    parser.add_argument("--corrupt", action="store_true")
    args = parser.parse_args()
    check(args.package, args.artifacts, args.force_build, args.corrupt)
