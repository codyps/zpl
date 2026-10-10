defmodule Zpl.Native do
  @moduledoc false
  # Checksums are generated from CI artifacts and included in the Hex archive.
  # Checkouts build their actual source, never an unrelated released binary.
  # https://hexdocs.pm/rustler_precompiled/RustlerPrecompiled.html
  @version Mix.Project.config()[:version]
  @checksum Path.expand("../../checksum-Elixir.Zpl.Native.exs", __DIR__)
  @external_resource @checksum

  use RustlerPrecompiled,
    otp_app: :zpl,
    crate: "zpl_elixir",
    version: @version,
    base_url:
      Application.compile_env(
        :zpl,
        :precompiled_base_url,
        "https://github.com/codyps/zpl/releases/download/zpl-v#{@version}"
      ),
    targets:
      ~w(x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc),
    nif_versions: ["2.15"],
    max_retries: 2,
    force_build:
      System.get_env("ZPL_BUILD") in ["1", "true"] or
        Application.compile_env(:rustler_precompiled, [:force_build, :zpl], false) or
        not File.exists?(@checksum),
    path: ".",
    load_from: {:zpl, "priv/native/zpl_elixir"},
    mode: if(Mix.env() == :prod, do: :release, else: :debug)

  def library_version, do: :erlang.nif_error(:nif_not_loaded)
  def defaults(_kind, _profile), do: :erlang.nif_error(:nif_not_loaded)
  def parse(_source, _syntax), do: :erlang.nif_error(:nif_not_loaded)
  def render(_source, _options, _limits), do: :erlang.nif_error(:nif_not_loaded)
  def scene_encode(_scene, _format, _limits), do: :erlang.nif_error(:nif_not_loaded)
  def rasterize(_scene, _limits), do: :erlang.nif_error(:nif_not_loaded)
  def document_pdf(_document, _limits), do: :erlang.nif_error(:nif_not_loaded)
end
