"""Release platform contracts shared by artifact validators.

Elixir starts with RustlerPrecompiled's default targets:
https://hexdocs.pm/rustler_precompiled/0.9.0/RustlerPrecompiled.html
Python uses CPython/MSVC on Windows; .NET uses Microsoft's portable RIDs:
https://learn.microsoft.com/en-us/dotnet/core/rid-catalog
"""

ELIXIR_TARGETS = (
    'aarch64-apple-darwin',
    'aarch64-unknown-linux-gnu',
    'aarch64-unknown-linux-musl',
    'arm-unknown-linux-gnueabihf',
    'riscv64gc-unknown-linux-gnu',
    'x86_64-apple-darwin',
    'x86_64-pc-windows-gnu',
    'x86_64-pc-windows-msvc',
    'x86_64-unknown-linux-gnu',
    'x86_64-unknown-linux-musl',
)

# Platform tags are validated individually, including ABI and libc. A count of
# wheels alone can accept duplicates for one platform while omitting another.
PYTHON_PLATFORMS = {
    'manylinux_2_28_x86_64', 'manylinux_2_28_aarch64',
    'manylinux_2_28_armv7l', 'manylinux_2_39_riscv64',
    'musllinux_1_2_x86_64', 'musllinux_1_2_aarch64', 'musllinux_1_2_armv7l',
    'macosx_x86_64', 'macosx_arm64',
    'win_amd64', 'win32', 'win_arm64',
}

DOTNET_NATIVES = {
    'win-x64': 'zpl_c.dll',
    'win-x86': 'zpl_c.dll',
    'win-arm64': 'zpl_c.dll',
    'linux-x64': 'libzpl_c.so',
    'linux-arm64': 'libzpl_c.so',
    'linux-arm': 'libzpl_c.so',
    'linux-musl-x64': 'libzpl_c.so',
    'linux-musl-arm64': 'libzpl_c.so',
    'linux-musl-arm': 'libzpl_c.so',
    'osx-x64': 'libzpl_c.dylib',
    'osx-arm64': 'libzpl_c.dylib',
}
