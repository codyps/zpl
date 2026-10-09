defmodule Zpl do
  @moduledoc """
  Local, byte-oriented ZPL parsing and rendering through the Rust `zpl` crate.

  Native operations return `{:ok, value}` or `{:error, %Zpl.Error{}}`. The bang
  variants raise that error. Input must be a binary; strings are passed as UTF-8
  bytes without implicitly selecting `^CI28`. All rendering and output work runs
  on BEAM dirty CPU schedulers.
  """

  @type config :: map() | keyword()
  @type profile :: :zd621 | :specification | :zq610_plus
  @type result(value) :: {:ok, value} | {:error, Zpl.Error.t()}

  @doc "The linked Rust renderer version."
  @spec library_version() :: String.t()
  def library_version, do: Zpl.Native.library_version()

  @doc "Native defaults for a profile, including its independently selectable compatibility flags."
  @spec options(profile()) :: map()
  def options(profile \\ :zd621), do: Zpl.Native.defaults(:options, profile) |> unwrap!()

  @doc "All native compatibility flags disabled (the strict specification behavior)."
  @spec compatibility() :: map()
  def compatibility, do: Zpl.Native.defaults(:compatibility, :specification) |> unwrap!()

  @doc "Native render resource budgets."
  @spec render_limits() :: map()
  def render_limits, do: Zpl.Native.defaults(:render_limits, :zd621) |> unwrap!()

  @doc "Native output resource budgets, independent of render budgets."
  @spec output_limits() :: map()
  def output_limits, do: Zpl.Native.defaults(:output_limits, :zd621) |> unwrap!()

  @doc "Default byte-valued parser prefixes and delimiter."
  @spec syntax() :: map()
  def syntax, do: Zpl.Native.defaults(:syntax, :zd621) |> unwrap!()

  @doc """
  Renders a complete binary into a document of scenes and warnings.

  Options accept `:profile`, `:width`, `:height`, `:dpi`, and `:compatibility`.
  Compatibility overrides merge into the selected profile. Limits accept every
  field of Rust's `render::Limits`. Unknown fields and invalid values fail.
  """
  @spec render(binary(), config(), config()) :: result(Zpl.Document.t())
  def render(source, options \\ [], limits \\ []) when is_binary(source) do
    options = Map.new(options)

    options =
      case Map.fetch(options, :compatibility) do
        {:ok, flags} -> Map.put(options, :compatibility, Map.new(flags))
        :error -> options
      end

    Zpl.Native.render(source, options, Map.new(limits))
  end

  @doc "Like `render/3`, raising on failure."
  @spec render!(binary(), config(), config()) :: Zpl.Document.t()
  def render!(source, options \\ [], limits \\ []),
    do: render(source, options, limits) |> unwrap!()

  @doc """
  Frames the entire input losslessly, preserving unknown commands and binary data.
  Returns elements with byte offsets and final syntax. Framing is neither operand
  validation nor authorization to send commands to a printer.
  """
  @spec parse(binary(), config()) :: result(Zpl.ParseResult.t())
  def parse(source, syntax \\ []) when is_binary(source),
    do: Zpl.Native.parse(source, Map.new(syntax))

  @doc "Like `parse/2`, raising on failure."
  @spec parse!(binary(), config()) :: Zpl.ParseResult.t()
  def parse!(source, syntax \\ []), do: parse(source, syntax) |> unwrap!()

  @doc false
  def unwrap!({:ok, value}), do: value
  def unwrap!({:error, error}), do: raise(error)
end
