defmodule Zpl.Document do
  @moduledoc """
  Rendered label scenes in source order and native warnings.

  The reference retains the original document. Editing `labels` or `warnings`
  does not alter PDF output. References are opaque, local to this VM.
  """
  defstruct [:labels, :warnings, :reference]
  @type t :: %__MODULE__{labels: [Zpl.Scene.t()], warnings: [String.t()], reference: reference()}

  @doc "Encodes all labels as a multipage PDF in source order."
  @spec pdf(t(), Zpl.config()) :: Zpl.result(binary())
  def pdf(%__MODULE__{reference: ref}, limits \\ []),
    do: Zpl.Native.document_pdf(ref, Map.new(limits))

  @spec pdf!(t(), Zpl.config()) :: binary()
  def pdf!(document, limits \\ []), do: pdf(document, limits) |> Zpl.unwrap!()
end
