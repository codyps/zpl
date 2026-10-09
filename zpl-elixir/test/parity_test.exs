defmodule Zpl.ParityTest do
  use ExUnit.Case, async: false

  @moduletag timeout: 120_000
  test "binding bytes equal direct Rust adapters for every profile" do
    directory = Path.join(System.tmp_dir!(), "zpl-parity-#{System.unique_integer([:positive])}")
    on_exit(fn -> File.rm_rf!(directory) end)

    {log, status} =
      System.cmd(
        "cargo",
        [
          "run",
          "--locked",
          "--quiet",
          "-p",
          "zpl_elixir",
          "--example",
          "reference",
          "--",
          directory
        ],
        stderr_to_stdout: true
      )

    assert status == 0, log
    source = "^XA^FO2,3^GB10,8,2^FS^XZ^XA^FO1,1^GB3,4,1^FS^XZ"

    for profile <- [:specification, :zd621, :zq610_plus] do
      document = Zpl.render!(source, profile: profile, width: 32, height: 24)
      assert document.warnings == []
      assert Zpl.Document.pdf!(document) == File.read!(Path.join(directory, "#{profile}.pdf"))

      for {scene, index} <- Enum.with_index(document.labels) do
        for format <- [:png, :svg, :pdf] do
          assert Zpl.Scene.encode!(scene, format) ==
                   File.read!(Path.join(directory, "#{profile}-#{index}.#{format}"))
        end

        assert Zpl.Scene.rasterize!(scene).pixels ==
                 File.read!(Path.join(directory, "#{profile}-#{index}.raw"))
      end
    end
  end
end
