defmodule ZplReadmeTest do
  # Synchronous: the examples write relative files, so the VM-wide working
  # directory changes to a temporary directory; ExUnit runs sync modules after
  # async ones (https://hexdocs.pm/ex_unit/ExUnit.Case.html#module-async).
  use ExUnit.Case, async: false
  import ExUnit.CaptureIO

  # Blocks run in order with bindings carried forward, like an IEx session.
  # A `<!-- readme-test: skip -->` line directly before a fence excludes a block.
  # Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
  @readme Path.expand("../README.md", __DIR__)
  @external_resource @readme
  @skip "<!-- readme-test: skip -->"

  defp blocks(text) do
    ~r/^```elixir[ \t]*\n(.*?)^```[ \t]*$/ms
    |> Regex.scan(text, return: :index)
    |> Enum.map(fn [{start, _}, {code_start, code_length}] ->
      prefix = binary_part(text, 0, start)
      previous = prefix |> String.trim_trailing("\n") |> String.split("\n") |> List.last()
      line = prefix |> String.split("\n") |> length()
      {line, binary_part(text, code_start, code_length), String.trim(previous || "") == @skip}
    end)
  end

  test "README examples run" do
    runnable = @readme |> File.read!() |> blocks() |> Enum.reject(&elem(&1, 2))
    assert runnable != []

    directory =
      Path.join(System.tmp_dir!(), "zpl-elixir-readme-#{System.unique_integer([:positive])}")

    File.mkdir_p!(directory)

    try do
      File.cd!(directory, fn ->
        capture_io(fn ->
          Enum.reduce(runnable, [], fn {line, code, _}, binding ->
            {_, binding} = Code.eval_string(code, binding, file: @readme, line: line + 1)
            binding
          end)
        end)
      end)
    after
      File.rm_rf!(directory)
    end
  end
end
