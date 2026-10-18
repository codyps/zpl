using System.Text;

namespace Codyps.Zpl;

public sealed unsafe class Fonts : IDisposable
{
    internal readonly FontsHandle Handle;
    internal readonly object Gate = new();
    internal void EnsureOpen() => ObjectDisposedException.ThrowIf(Handle.IsClosed, this);

    public Fonts()
    {
        Native.Check(Native.zpl_fonts_new(out var value));
        Handle = new(value);
    }

    // The native library copies font bytes; managed input need not stay pinned.
    public void AddTrueType(uint id, ReadOnlySpan<byte> data, Hinting hinting = Hinting.Native)
    {
        lock (Gate)
        {
            EnsureOpen();
            fixed (byte* bytes = data)
                Native.Check(Native.zpl_fonts_insert_truetype(Handle, id, bytes, (nuint)data.Length, hinting));
        }
    }

    public void AddTrueType(string name, ReadOnlySpan<byte> data, Hinting hinting = Hinting.Native)
    {
        ArgumentNullException.ThrowIfNull(name);
        byte[] utf8 = Encoding.UTF8.GetBytes(name);
        lock (Gate)
        {
            EnsureOpen();
            fixed (byte* filename = utf8)
            fixed (byte* bytes = data)
                Native.Check(Native.zpl_fonts_insert_named_truetype(Handle, filename,
                    (nuint)utf8.Length, bytes, (nuint)data.Length, hinting));
        }
    }

    public void Dispose() { lock (Gate) Handle.Dispose(); }
}
