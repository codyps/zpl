using System.Text;

namespace Codyps.Zpl;

public sealed record ParsedElement(ElementKind Kind, nuint ByteOffset, byte[] Data);
public sealed record ParseResult(IReadOnlyList<ParsedElement> Elements, Syntax FinalSyntax);

public static unsafe class Parser
{
    public static ParseResult Parse(string zpl, Syntax? syntax = null)
    {
        ArgumentNullException.ThrowIfNull(zpl);
        return Parse(Encoding.UTF8.GetBytes(zpl), syntax);
    }

    // Framing only, not validation or authorization. Bound input size upstream.
    public static ParseResult Parse(ReadOnlySpan<byte> zpl, Syntax? syntax = null)
    {
        var nativeSyntax = syntax.GetValueOrDefault();
        fixed (byte* data = zpl)
        {
            Native.Check(Native.zpl_parse(data, (nuint)zpl.Length, syntax.HasValue ? &nativeSyntax : null, out var value));
            using var parsed = new ParsedHandle(value);
            var elements = new ParsedElement[checked((int)Native.zpl_parsed_count(parsed))];
            for (int i = 0; i < elements.Length; i++)
            {
                Native.Check(Native.zpl_parsed_element(parsed, (nuint)i, out var element));
                elements[i] = new(element.Kind, element.Offset, element.Data.Copy());
            }
            Native.Check(Native.zpl_parsed_syntax(parsed, out var finalSyntax));
            return new(elements, finalSyntax);
        }
    }
}
