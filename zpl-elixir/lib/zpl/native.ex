defmodule Zpl.Native do
  @moduledoc false
  # https://hexdocs.pm/rustler/Rustler.html
  use Rustler,
    otp_app: :zpl,
    crate: "zpl_elixir",
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
