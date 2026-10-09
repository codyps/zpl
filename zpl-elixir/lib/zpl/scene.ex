defmodule Zpl.Scene do
  @moduledoc """
  An immutable rendered label with width, height, and DPI metadata.

  The native reference owns the scene independently of the document's Elixir
  lifetime. Metadata fields are informational: editing them does not change the
  native scene. References are local to the VM and cannot be persisted or sent
  to another node. Output limits accept maps or keyword lists.
  """
  defstruct [:width, :height, :dpi, :reference]

  @type t :: %__MODULE__{
          width: pos_integer(),
          height: pos_integer(),
          dpi: pos_integer(),
          reference: reference()
        }

  @doc "Encodes one label as a PNG, SVG, or PDF binary."
  @spec encode(t(), :png | :svg | :pdf, Zpl.config()) :: Zpl.result(binary())
  def encode(%__MODULE__{reference: ref}, format, limits \\ []),
    do: Zpl.Native.scene_encode(ref, format, Map.new(limits))

  @spec encode!(t(), :png | :svg | :pdf, Zpl.config()) :: binary()
  def encode!(scene, format, limits \\ []), do: encode(scene, format, limits) |> Zpl.unwrap!()

  @spec png(t(), Zpl.config()) :: Zpl.result(binary())
  def png(scene, limits \\ []), do: encode(scene, :png, limits)
  @spec svg(t(), Zpl.config()) :: Zpl.result(binary())
  def svg(scene, limits \\ []), do: encode(scene, :svg, limits)
  @spec pdf(t(), Zpl.config()) :: Zpl.result(binary())
  def pdf(scene, limits \\ []), do: encode(scene, :pdf, limits)

  @doc "Rasterizes a label to raw grayscale bytes."
  @spec rasterize(t(), Zpl.config()) :: Zpl.result(Zpl.Raster.t())
  def rasterize(%__MODULE__{reference: ref}, limits \\ []),
    do: Zpl.Native.rasterize(ref, Map.new(limits))

  @spec rasterize!(t(), Zpl.config()) :: Zpl.Raster.t()
  def rasterize!(scene, limits \\ []), do: rasterize(scene, limits) |> Zpl.unwrap!()
end
