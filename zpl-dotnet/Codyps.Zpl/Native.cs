using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace Codyps.Zpl;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeBytes
{
    internal readonly IntPtr Data;
    internal readonly nuint Length;
    internal byte[] Copy()
    {
        var bytes = new byte[checked((int)Length)];
        if (bytes.Length > 0) Marshal.Copy(Data, bytes, 0, bytes.Length);
        return bytes;
    }
    internal string Text() => Encoding.UTF8.GetString(Copy());
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeError { internal int Status; internal nuint Offset; internal NativeBytes Message; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeElement { internal ElementKind Kind; internal nuint Offset; internal NativeBytes Data; }
[StructLayout(LayoutKind.Sequential)]
public struct SceneInfo { public uint Width; public uint Height; public uint Dpi; }

public sealed class ZplException : Exception
{
    public Status Status { get; }
    public nuint? ByteOffset { get; }
    internal ZplException(NativeError error) : base(error.Message.Text())
    {
        Status = (Status)error.Status;
        ByteOffset = error.Offset == unchecked((nuint)(-1)) ? null : error.Offset;
    }
}

// SafeHandle guards each P/Invoke. Owners also lock across borrowed-view copies,
// preventing concurrent Dispose from releasing the underlying storage mid-copy.
// https://learn.microsoft.com/en-us/dotnet/standard/native-interop/best-practices
internal abstract class ZplHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    protected ZplHandle(IntPtr value) : base(true) => SetHandle(value);
}
internal sealed class DocumentHandle(IntPtr value) : ZplHandle(value)
{
    protected override bool ReleaseHandle() { Native.zpl_document_free(handle); return true; }
}
internal sealed class SceneHandle(IntPtr value) : ZplHandle(value)
{
    protected override bool ReleaseHandle() { Native.zpl_scene_free(handle); return true; }
}
internal sealed class BufferHandle(IntPtr value) : ZplHandle(value)
{
    protected override bool ReleaseHandle() { Native.zpl_buffer_free(handle); return true; }
}
internal sealed class ParsedHandle(IntPtr value) : ZplHandle(value)
{
    protected override bool ReleaseHandle() { Native.zpl_parsed_free(handle); return true; }
}
internal sealed class FontsHandle(IntPtr value) : ZplHandle(value)
{
    protected override bool ReleaseHandle() { Native.zpl_fonts_free(handle); return true; }
}

internal static unsafe class Native
{
    private const string Library = "zpl_c";
    internal const uint AbiVersion = 4;
    static Native()
    {
#if NETFRAMEWORK
        if (Environment.OSVersion.Platform != PlatformID.Win32NT || !Environment.Is64BitProcess)
            throw new PlatformNotSupportedException("Codyps.Zpl requires Windows x64 on .NET Framework.");
#endif
        uint actual = zpl_abi_version();
        if (actual != AbiVersion)
            throw new NotSupportedException($"ZPL ABI {actual} is incompatible with expected ABI {AbiVersion}.");
    }
    internal static void Check(int status)
    {
        // Copy thread-local error text before any other fallible native call.
        if (status != 0) throw new ZplException(zpl_last_error());
    }
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern uint zpl_abi_version();
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern NativeBytes zpl_library_version();
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern NativeError zpl_last_error();
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_options_init(Profile profile, out RenderOptions value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_render_limits_init(out RenderLimits value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_output_limits_init(out OutputLimits value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_syntax_init(out Syntax value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_render(byte* data, nuint len, RenderOptions* options, RenderLimits* limits, out IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_render_with_fonts(byte* data, nuint len, RenderOptions* options, FontsHandle fonts, RenderLimits* limits, out IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void zpl_document_free(IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern nuint zpl_document_label_count(DocumentHandle value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern nuint zpl_document_warning_count(DocumentHandle value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_document_warning(DocumentHandle value, nuint index, out NativeBytes bytes);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_document_label(DocumentHandle value, nuint index, out IntPtr scene);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_document_pdf(DocumentHandle value, OutputLimits* limits, out IntPtr buffer);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void zpl_scene_free(IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_scene_info(SceneHandle value, out SceneInfo info);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_scene_encode(SceneHandle value, OutputFormat format, OutputLimits* limits, out IntPtr buffer);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern NativeBytes zpl_buffer_bytes(BufferHandle value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void zpl_buffer_free(IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_parse(byte* data, nuint len, Syntax* syntax, out IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void zpl_parsed_free(IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern nuint zpl_parsed_count(ParsedHandle value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_parsed_element(ParsedHandle value, nuint index, out NativeElement element);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_parsed_syntax(ParsedHandle value, out Syntax syntax);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_fonts_new(out IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void zpl_fonts_free(IntPtr value);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_fonts_insert_truetype(FontsHandle value, uint id, byte* data, nuint len, Hinting hinting);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int zpl_fonts_insert_named_truetype(FontsHandle value, byte* name, nuint nameLen, byte* data, nuint len, Hinting hinting);
}
