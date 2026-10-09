defmodule ZplTest do
  use ExUnit.Case, async: true
  alias Zpl.{Document, Error, Scene}

  # Binding contracts: docs/parser-coverage.md and docs/local-renderer.md.
  @source "^XA^FO2,3^GB4,2,2^FS^XZ"
  @options [profile: :specification, width: 16, height: 12]

  test "release package and native renderer versions agree" do
    version = System.get_env("RELEASE_VERSION")

    if version not in [nil, ""] do
      assert Zpl.library_version() == version
      assert to_string(Application.spec(:zpl, :vsn)) == version
    end
  end

  test "output signatures, dimensions and exact native-origin pixels" do
    assert {:ok, document} = Zpl.render(@source, @options)
    assert document.warnings == []
    assert [scene] = document.labels
    assert {scene.width, scene.height, scene.dpi} == {16, 12, 203}
    assert {:ok, <<137, "PNG\r\n", 26, "\n", _::binary>> = png} = Scene.png(scene)
    assert <<_::binary-size(16), 16::32, 12::32, _::binary>> = png
    assert {:ok, svg} = Scene.svg(scene)
    assert svg =~ "<svg"
    assert Scene.pdf(scene) == Document.pdf(document)
    assert {:ok, <<"%PDF-", _::binary>>} = Scene.pdf(scene)
    raster = Scene.rasterize!(scene)
    assert {raster.width, raster.height} == {16, 12}

    expected =
      for y <- 0..11, x <- 0..15, into: <<>> do
        if x in 2..5 and y in 3..4, do: <<0>>, else: <<255>>
      end

    assert raster.pixels == expected
  end

  test "multipage documents and resources surviving their creating process" do
    {scene, png} =
      Task.async(fn ->
        document = Zpl.render!(@source <> "^XA^PW8^LL9^XZ", @options)
        assert [scene, %{width: 8, height: 9}] = document.labels
        assert Document.pdf!(document) =~ "/Count 2"
        {scene, Scene.encode!(scene, :png)}
      end)
      |> Task.await()

    :erlang.garbage_collect()
    assert Scene.encode!(scene, :png) == png
    results = 1..12 |> Task.async_stream(fn _ -> Scene.encode!(scene, :png) end) |> Enum.to_list()
    assert Enum.all?(results, &(&1 == {:ok, png}))
  end

  test "native profile defaults and independent compatibility overrides" do
    assert %{width: 832, height: 1218, dpi: 203} = Zpl.options()
    assert %{width: 384, height: 2030} = Zpl.options(:zq610_plus)
    assert Zpl.options().compatibility.qr_printer_mask_selection
    refute Zpl.options(:specification).compatibility.qr_printer_mask_selection
    refute Zpl.compatibility().box_zero_thickness_as_one
    source = "^XA^FO2,3^GB4,2,0^FS^XZ"
    assert {:error, %Error{stage: :render}} = Zpl.render(source, @options)

    options =
      Keyword.put(@options, :compatibility,
        box_zero_thickness_as_one: true,
        macro_pdf417_file_id: {1, 2, 3}
      )

    assert {:ok, document} = Zpl.render(source, options)
    assert Scene.rasterize!(hd(document.labels)).pixels =~ <<0>>
    # Full defaults maps round-trip through every configuration field.
    assert {:ok, _} = Zpl.render(@source, Zpl.options(:specification), Zpl.render_limits())
    assert {:ok, _} = Scene.png(hd(document.labels), Zpl.output_limits())
  end

  test "warnings remain visible" do
    document = Zpl.render!("^XA^FDHello^FS^XZ", @options)
    assert Enum.any?(document.warnings, &String.contains?(&1, "captured bitmap strikes"))
  end

  test "lossless framing of binary payloads, unknown commands, UTF-8 and byte offsets" do
    for source <- [
          " \n^XA^ZZunknown^GFB,4,4,4,^~" <> <<0, 255>> <> "^FS" <> <<3>>,
          "^XA^CI28^FDé^FS^XZ"
        ] do
      result = Zpl.parse!(source)
      assert IO.iodata_to_binary(Enum.map(result.elements, & &1.data)) == source

      assert Enum.reduce(result.elements, 0, fn element, offset ->
               assert element.offset == offset
               offset + byte_size(element.data)
             end) == byte_size(source)
    end
  end

  test "syntax changes carry into a subsequent complete buffer" do
    result = Zpl.parse!("^CC!!XA!CD;!FO1;2!XZ")
    assert result.syntax == %{format_prefix: ?!, control_prefix: ?~, delimiter: ?;}
    assert Enum.map(Zpl.parse!("!XA!XZ", result.syntax).elements, & &1.data) == ["!XA", "!XZ"]
    assert [%{kind: :format_command}] = Zpl.parse!("!XA", format_prefix: ?!).elements
    assert Zpl.parse!("", Zpl.syntax()).elements == []
  end

  test "structured diagnostics and bang exceptions" do
    assert {:error, %Error{stage: :parse, offset: 3, kind: "TruncatedBinaryData"}} =
             Zpl.parse("^XA^GFB,4,4,4,ab")

    assert {:error, %Error{stage: :render, offset: 3}} = Zpl.render("^XA^ZZ^XZ", @options)
    assert_raise Error, fn -> Zpl.parse!("^XA^GFB,4,4,4,ab") end
    assert_raise Error, fn -> Zpl.render!("", @options) end
  end

  test "render and output limits remain independent and enforced" do
    for limits <- [[input_bytes: 1], [labels: 0], [pixels: 1], [segments: 0], [dimension: 1]] do
      assert {:error, %Error{stage: :render}} = Zpl.render(@source, @options, limits)
    end

    document = Zpl.render!(@source, @options)
    scene = hd(document.labels)

    for format <- [:png, :svg, :pdf] do
      assert {:error, %Error{stage: :output}} = Scene.encode(scene, format, pixels: 1)
    end

    assert {:error, %Error{stage: :output}} = Scene.rasterize(scene, pixels: 1)
    assert {:error, %Error{stage: :output}} = Document.pdf(document, pages: 0)
    assert {:error, %Error{stage: :render}} = Zpl.render("^XA^PW100^XZ", @options, dimension: 32)
  end

  test "invalid configurations fail without silently ignoring fields" do
    for options <- [
          [width: 0],
          [height: -1],
          [dpi: 0],
          [width: 1_099_511_627_776],
          [profile: :missing],
          [typo: 1],
          [compatibility: [typo: true]],
          [compatibility: [macro_pdf417_file_id: {1, 2}]],
          [compatibility: [macro_pdf417_file_id: {65_536, 2, 3}]],
          [compatibility: [retail_guard_extension_dots: 65_536]]
        ] do
      assert {:error, %Error{stage: :argument}} = Zpl.render(@source, options)
    end

    assert {:error, %Error{stage: :argument}} = Zpl.parse(@source, delimiter: 256)
    assert {:error, %Error{stage: :argument}} = Zpl.render(@source, @options, number_abs: -1.0)
    assert {:error, %Error{stage: :argument}} = Zpl.render(@source, @options, unknown: 1)
    scene = hd(Zpl.render!(@source, @options).labels)
    assert {:error, %Error{stage: :argument}} = Scene.png(scene, coordinate_abs: -1.0)
    assert {:error, %Error{stage: :argument}} = Scene.encode(scene, :unknown)
    assert Zpl.library_version() =~ ~r/^\d+\.\d+\.\d+/
  end
end
