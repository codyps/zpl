defmodule Zpl.Error do
  @moduledoc "Native diagnostic with stage, message, and optional byte offset and parse kind."
  defexception [:stage, :message, :offset, :kind]

  @type t :: %__MODULE__{
          stage: :argument | :parse | :render | :output,
          message: String.t(),
          offset: non_neg_integer() | nil,
          kind: String.t() | nil
        }
end

defmodule Zpl.Element do
  @moduledoc "A lossless parser element. `offset` is a byte offset and `data` is a binary."
  defstruct [:kind, :offset, :data]
  @type t :: %__MODULE__{kind: atom(), offset: non_neg_integer(), data: binary()}
end

defmodule Zpl.ParseResult do
  @moduledoc "Parsed elements in source order and the final parser syntax."
  defstruct [:elements, :syntax]
  @type t :: %__MODULE__{elements: [Zpl.Element.t()], syntax: map()}
end

defmodule Zpl.Raster do
  @moduledoc "Row-major grayscale pixels: one byte per dot, 0 black and 255 white."
  defstruct [:width, :height, :pixels]
  @type t :: %__MODULE__{width: pos_integer(), height: pos_integer(), pixels: binary()}
end
