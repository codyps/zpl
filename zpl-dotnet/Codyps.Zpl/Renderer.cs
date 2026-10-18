using System.Text;

namespace Codyps.Zpl;

public static unsafe class Renderer
{
    public static string LibraryVersion => Native.zpl_library_version().Text();

    public static Document Render(string zpl, RenderOptions? options = null,
        RenderLimits? limits = null, Fonts? fonts = null)
    {
        ArgumentNullException.ThrowIfNull(zpl);
        return Render(Encoding.UTF8.GetBytes(zpl), options, limits, fonts);
    }

    // Byte input preserves legacy encodings, binary graphics and embedded NULs.
    public static Document Render(ReadOnlySpan<byte> zpl, RenderOptions? options = null,
        RenderLimits? limits = null, Fonts? fonts = null)
    {
        var nativeOptions = options.GetValueOrDefault();
        var nativeLimits = limits.GetValueOrDefault();
        fixed (byte* data = zpl)
        {
            IntPtr value;
            if (fonts is null)
                Native.Check(Native.zpl_render(data, (nuint)zpl.Length,
                    options.HasValue ? &nativeOptions : null,
                    limits.HasValue ? &nativeLimits : null, out value));
            else
                lock (fonts.Gate)
                {
                    fonts.EnsureOpen();
                    Native.Check(Native.zpl_render_with_fonts(data, (nuint)zpl.Length,
                        options.HasValue ? &nativeOptions : null, fonts.Handle,
                        limits.HasValue ? &nativeLimits : null, out value));
                }
            return new Document(value);
        }
    }
}

public sealed unsafe class Document : IDisposable
{
    private readonly DocumentHandle handle;
    private readonly object gate = new();
    internal Document(IntPtr value) => handle = new(value);
    private void EnsureOpen() => ObjectDisposedException.ThrowIf(handle.IsClosed, this);

    public int LabelCount
    {
        get { lock (gate) { EnsureOpen(); return checked((int)Native.zpl_document_label_count(handle)); } }
    }

    public IReadOnlyList<string> Warnings
    {
        get
        {
            lock (gate)
            {
                EnsureOpen();
                var warnings = new string[checked((int)Native.zpl_document_warning_count(handle))];
                for (int i = 0; i < warnings.Length; i++)
                {
                    Native.Check(Native.zpl_document_warning(handle, (nuint)i, out var bytes));
                    warnings[i] = bytes.Text();
                }
                return warnings;
            }
        }
    }

    public Scene GetLabel(int index)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(index);
        lock (gate)
        {
            EnsureOpen();
            Native.Check(Native.zpl_document_label(handle, (nuint)index, out var scene));
            return new Scene(scene);
        }
    }

    public byte[] ToPdf(OutputLimits? limits = null)
    {
        var nativeLimits = limits.GetValueOrDefault();
        lock (gate)
        {
            EnsureOpen();
            Native.Check(Native.zpl_document_pdf(handle, limits.HasValue ? &nativeLimits : null, out var value));
            using var buffer = new BufferHandle(value);
            return Native.zpl_buffer_bytes(buffer).Copy();
        }
    }

    public void Dispose() { lock (gate) handle.Dispose(); }
}

public sealed unsafe class Scene : IDisposable
{
    private readonly SceneHandle handle;
    private readonly object gate = new();
    internal Scene(IntPtr value) => handle = new(value);
    private void EnsureOpen() => ObjectDisposedException.ThrowIf(handle.IsClosed, this);

    public SceneInfo Info
    {
        get
        {
            lock (gate)
            {
                EnsureOpen();
                Native.Check(Native.zpl_scene_info(handle, out var info));
                return info;
            }
        }
    }

    public byte[] Encode(OutputFormat format, OutputLimits? limits = null)
    {
        var nativeLimits = limits.GetValueOrDefault();
        lock (gate)
        {
            EnsureOpen();
            Native.Check(Native.zpl_scene_encode(handle, format, limits.HasValue ? &nativeLimits : null, out var value));
            using var buffer = new BufferHandle(value);
            return Native.zpl_buffer_bytes(buffer).Copy();
        }
    }

    public byte[] ToPng(OutputLimits? limits = null) => Encode(OutputFormat.Png, limits);
    public string ToSvg(OutputLimits? limits = null) => Encoding.UTF8.GetString(Encode(OutputFormat.Svg, limits));
    public byte[] ToPdf(OutputLimits? limits = null) => Encode(OutputFormat.Pdf, limits);
    public byte[] ToGrayscale(OutputLimits? limits = null) => Encode(OutputFormat.Gray8, limits);
    public void Dispose() { lock (gate) handle.Dispose(); }
}
