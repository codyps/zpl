namespace Codyps.Zpl;

public enum Profile : uint { Zd621 = 0, Specification = 1, Zq610Plus = 2 }
public enum OutputFormat : uint { Png = 0, Svg = 1, Pdf = 2, Gray8 = 3 }
public enum Hinting : uint { None = 0, Native = 1 }
public enum Status { InvalidArgument = 1, ParseError = 2, RenderError = 3, OutputError = 4, Panic = 5, FontError = 6 }
public enum ElementKind : uint { BeforeFirstCommand = 0, FormatCommand = 1, ControlCommand = 2, ControlCharacter = 3 }

// Zero-initialized structs are not native defaults. Always start with Create().
public partial struct RenderOptions
{
    public static RenderOptions Create(Profile profile = Profile.Zd621)
    {
        Native.Check(Native.zpl_options_init(profile, out var value));
        return value;
    }
}

public partial struct RenderLimits
{
    public static RenderLimits Create()
    {
        Native.Check(Native.zpl_render_limits_init(out var value));
        return value;
    }
}

public partial struct OutputLimits
{
    public static OutputLimits Create()
    {
        Native.Check(Native.zpl_output_limits_init(out var value));
        return value;
    }
}

public partial struct Syntax
{
    public static Syntax Create()
    {
        Native.Check(Native.zpl_syntax_init(out var value));
        return value;
    }
}

public partial struct OptionalUInt32
{
    public static OptionalUInt32 Some(uint value) => new() { Present = 1, Value = value };
}

public partial struct OptionalUInt16
{
    public static OptionalUInt16 Some(ushort value) => new() { Present = 1, Value = value };
}

public partial struct OptionalFileId
{
    public static OptionalFileId Some(ushort first, ushort second, ushort third) =>
        new() { Present = 1, Value0 = first, Value1 = second, Value2 = third };
}
