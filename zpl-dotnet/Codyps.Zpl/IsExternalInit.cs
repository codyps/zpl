// Records need this compiler marker on targets predating .NET 5. It is internal
// so consumers can use their own marker without an assembly/type conflict.
#if !NET5_0_OR_GREATER
namespace System.Runtime.CompilerServices
{
    internal static class IsExternalInit { }
}
#endif
