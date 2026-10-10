defmodule Zpl.MixProject do
  use Mix.Project

  def project do
    [
      app: :zpl,
      version: "0.3.0",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: [
        {:rustler, "~> 0.38.0", runtime: false},
        # 0.10 requires Elixir 1.16; retain our documented Elixir 1.15 minimum.
        {:rustler_precompiled, "~> 0.9.0", runtime: false}
      ],
      aliases: ["hex.build": [&ensure_bundled_sources/1, "hex.build"]],
      description: "Local ZPL parsing and rendering powered by the Rust zpl crate",
      package: [
        licenses: ["OSL-3.0"],
        links: %{"GitHub" => "https://github.com/codyps/zpl"},
        files:
          ~w(lib src native test examples Cargo.toml Cargo.lock mix.exs README.md LICENSE .formatter.exs checksum-Elixir.Zpl.Native.exs)
      ]
    ]
  end

  defp ensure_bundled_sources(_args) do
    unless File.exists?("native/zpl/Cargo.toml") and File.exists?("Cargo.lock") do
      Mix.raise("Stage the source package with scripts/elixir-package.py before mix hex.build")
    end
  end

  def application, do: [extra_applications: [:logger]]
end
