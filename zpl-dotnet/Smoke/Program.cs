using System.Text;
using Codyps.Zpl;

static void Assert(bool condition, string message)
{
    if (!condition) throw new Exception(message);
}
static T Throws<T>(Action action) where T : Exception
{
    try { action(); }
    catch (T error) { return error; }
    throw new Exception($"Expected {typeof(T).Name}");
}

string expectedVersion = args[0];
Assert(Renderer.LibraryVersion == expectedVersion, "Linked renderer version");
const string label = "^XA^FO2,2^GB20,10,1^FS^XZ";
using (var defaults = Renderer.Render(label))
    Assert(defaults.LabelCount == 1, "Omitted configuration uses native defaults");

foreach (Profile profile in Enum.GetValues(typeof(Profile)))
{
    var profileOptions = RenderOptions.Create(profile);
    Assert(profileOptions.Dpi == (profile == Profile.Gx430t ? 300u : 203u) && profileOptions.Width > 0, "Profile initialized");
    using var profileDocument = Renderer.Render(label, profileOptions);
    Assert(profileDocument.LabelCount == 1, "Profile renders");
}

var options = RenderOptions.Create(Profile.Specification);
options.Width = 64;
options.Height = 32;
// Exercise optional-field alignment and the end of the large compatibility struct.
options.Compatibility.PreviewWidthQuantum = OptionalUInt32.Some(64);
options.Compatibility.RetailGuardExtensionDots = OptionalUInt16.Some(4);
options.Compatibility.MacroPdf417FileId = OptionalFileId.Some(1, 2, 3);
using var document = Renderer.Render(Encoding.UTF8.GetBytes(label + label), options, RenderLimits.Create());
Assert(document.LabelCount == 2, "Multiple labels");
_ = document.Warnings;
Assert(Encoding.ASCII.GetString(document.ToPdf()).StartsWith("%PDF-"), "Multipage PDF");
using var scene = document.GetLabel(0);
document.Dispose(); // Owned scenes must survive their document.
Assert(scene.Info.Width == 64 && scene.Info.Height == 32 && scene.Info.Dpi == 203, "Scene dimensions");
Assert(scene.ToPng().AsSpan(0, 8).SequenceEqual(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }), "PNG signature");
Assert(scene.ToSvg().Contains("<svg"), "SVG output");
Assert(Encoding.ASCII.GetString(scene.ToPdf()).StartsWith("%PDF-"), "Single-page PDF");
byte[] pixels = scene.ToGrayscale();
Assert(pixels.Length == 64 * 32 && pixels.Contains((byte)0) && pixels.Contains((byte)255), "Grayscale output");

var outputLimits = OutputLimits.Create();
outputLimits.Pixels = 1;
var outputError = Throws<ZplException>(() => scene.ToPng(outputLimits));
Assert(outputError.Status == Status.OutputError && !string.IsNullOrEmpty(outputError.Message), "Native output error");
string savedMessage = outputError.Message;
_ = scene.ToPng();
Assert(outputError.Message == savedMessage, "Error copied before next native call");

var renderLimits = RenderLimits.Create();
renderLimits.InputBytes = 1;
Assert(Throws<ZplException>(() => Renderer.Render(label, options, renderLimits)).Status == Status.RenderError, "Render budget");
var invalidOptions = options;
invalidOptions.Compatibility.Code128AboveTextKeepsBarOrigin = 2;
Assert(Throws<ZplException>(() => Renderer.Render(label, invalidOptions)).Status == Status.InvalidArgument, "Tail flag layout");
using (var live = Renderer.Render(label, options))
    Assert(Throws<ZplException>(() => live.GetLabel(99)).Status == Status.InvalidArgument, "Bad label index");

byte[] input = Encoding.GetEncoding(28591).GetBytes("prefix^XA^FO1,1^FDa\0\xffb^FS^XZ");
var parsed = Parser.Parse(input);
Assert(parsed.Elements.SelectMany(element => element.Data).SequenceEqual(input), "Lossless binary parser");
Assert(parsed.Elements[0].ByteOffset == 0 && parsed.FinalSyntax.FormatPrefix == (byte)'^', "Parser offsets and syntax");
var syntax = Syntax.Create();
syntax.FormatPrefix = (byte)'!';
Assert(Parser.Parse("!XA!XZ", syntax).Elements.Count == 2, "Custom syntax");
Assert(Parser.Parse(ReadOnlySpan<byte>.Empty).Elements.Count == 0, "Empty input");
var parseError = Throws<ZplException>(() => Parser.Parse("^XA^GFB,2,2,1,"));
Assert(parseError.Status == Status.ParseError && parseError.ByteOffset.HasValue, "Parser byte offset");

using (var fonts = new Fonts())
{
    using var withFonts = Renderer.Render(label, options, fonts: fonts);
    Assert(withFonts.LabelCount == 1, "Font collection render");
    byte[] font = File.ReadAllBytes(args[1]);
    fonts.AddTrueType((uint)'Z', font, Hinting.None);
    fonts.AddTrueType("R:PROBE.TTF", font, Hinting.None);
    Array.Clear(font, 0, font.Length); // Registration copies input, rather than retaining a pin.
    using (var idDocument = Renderer.Render("^XA^FO2,2^AZN,24,24^FD!^FS^XZ", options, fonts: fonts))
    using (var namedDocument = Renderer.Render("^XA^FO2,2^A@N,24,24,R:PROBE.TTF^FD!^FS^XZ", options, fonts: fonts))
    using (var idScene = idDocument.GetLabel(0))
    using (var namedScene = namedDocument.GetLabel(0))
    {
        byte[] idPixels = idScene.ToGrayscale();
        Assert(idPixels.Contains((byte)0), "Custom TrueType font renders");
        Assert(idPixels.SequenceEqual(namedScene.ToGrayscale()), "ID and virtual filename fonts agree");
    }
    Throws<ZplException>(() => fonts.AddTrueType((uint)'Z', new byte[] { 0, 1, 2 }));
    Throws<ZplException>(() => fonts.AddTrueType("R:FONT.TTF", new byte[] { 0, 1, 2 }));
    fonts.Dispose();
    Throws<ObjectDisposedException>(() => Renderer.Render(label, options, fonts: fonts));
}

Parallel.For(0, 16, _ =>
{
    Assert(Throws<ZplException>(() => Renderer.Render(label, invalidOptions)).Status == Status.InvalidArgument,
        "Concurrent thread-local diagnostics");
    using var concurrent = Renderer.Render(label, options);
    using var concurrentScene = concurrent.GetLabel(0);
    Assert(concurrentScene.ToGrayscale().SequenceEqual(pixels), "Concurrent renders");
});
Throws<ObjectDisposedException>(() => document.GetLabel(0));
scene.Dispose();
Throws<ObjectDisposedException>(() => scene.ToPng());
Assert(pixels.Length == 2048, "Copied bytes survive native disposal");
Console.WriteLine($"Codyps.Zpl {expectedVersion}: package consumer checks passed");
