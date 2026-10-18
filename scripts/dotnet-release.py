#!/usr/bin/env python3
"""Validate the complete NuGet artifact and gate immutable publication.

NuGet v3 flat container and package signing:
https://learn.microsoft.com/en-us/nuget/api/package-base-address-resource
https://learn.microsoft.com/en-us/nuget/reference/signed-packages-reference
"""
import argparse
import io
from pathlib import Path
import re
import tomllib
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

from renderer_release import release_version

PACKAGE = 'Codyps.Zpl'
PROJECT = 'zpl-dotnet/Codyps.Zpl/Codyps.Zpl.csproj'
NATIVES = {
    'win-x64': 'zpl_c.dll',
    'linux-x64': 'libzpl_c.so',
    'linux-arm64': 'libzpl_c.so',
    'osx-x64': 'libzpl_c.dylib',
    'osx-arm64': 'libzpl_c.dylib',
}
MANAGED_ASSETS = {
    'ref/net8.0/Codyps.Zpl.dll', 'runtimes/any/lib/net8.0/Codyps.Zpl.dll',
    'ref/netstandard2.0/Codyps.Zpl.dll', 'runtimes/any/lib/netstandard2.0/Codyps.Zpl.dll',
    'ref/net472/Codyps.Zpl.dll', 'runtimes/any/lib/net472/Codyps.Zpl.dll',
    'lib/net472/Codyps.Zpl.dll',
    'build/Codyps.Zpl.targets', 'buildTransitive/Codyps.Zpl.targets',
}


def check_version(version):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('Expected a stable renderer version')


def project_version(root):
    project = ET.parse(root / PROJECT)
    if project.findtext('./PropertyGroup/PackageId') != PACKAGE:
        raise ValueError('Unexpected NuGet package ID')
    versions = project.findall('./PropertyGroup/Version')
    if len(versions) != 1:
        raise ValueError('Expected exactly one NuGet version')
    return versions[0].text


def stamp_text(root, version):
    check_version(version)
    old = project_version(root)
    text, count = re.subn(rf'<Version>{re.escape(old)}</Version>',
                          f'<Version>{version}</Version>', (root / PROJECT).read_text())
    if count != 1:
        raise ValueError('Expected exactly one NuGet version')
    return text


def prepare(root):
    version = release_version(root)
    if version is None:
        print('released=false')
        return
    check_version(version)
    renderer = tomllib.loads((root / 'zpl/Cargo.toml').read_text())['package']['version']
    native = tomllib.loads((root / 'zpl-c/Cargo.toml').read_text())['package']['version']
    if project_version(root) != version or renderer != version or native != version:
        raise ValueError('NuGet, C binding and renderer release versions must match')
    print(f'released=true\nversion={version}')


def payload(data, version, required_rids=None):
    check_version(version)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise ValueError('Duplicate NuGet archive entries')
        nuspec = ET.fromstring(archive.read(f'{PACKAGE}.nuspec'))
        # The schema namespace can vary independently of the package metadata.
        for element in nuspec.iter():
            element.tag = element.tag.rsplit('}', 1)[-1]
        if nuspec.findtext('metadata/id') != PACKAGE or nuspec.findtext('metadata/version') != version:
            raise ValueError('Unexpected NuGet package ID/version')
        if nuspec.findtext('metadata/license') != 'OSL-3.0':
            raise ValueError('Unexpected NuGet license')
        native_paths = {f'runtimes/{rid}/native/{name}' for rid, name in NATIVES.items()}
        required = {f'runtimes/{rid}/native/{NATIVES[rid]}' for rid in
                    (NATIVES if required_rids is None else required_rids)}
        required |= MANAGED_ASSETS | {'README.md', 'LICENSE'}
        if not required.issubset(names):
            raise ValueError(f'Incomplete NuGet package: missing {sorted(required - set(names))}')
        actual_native = {name for name in names if '/native/' in name}
        if actual_native - native_paths or (required_rids is None and actual_native != native_paths):
            raise ValueError('Unexpected NuGet native binaries')
        if any(not archive.read(name) for name in required):
            raise ValueError('Empty NuGet payload')
        # NuGet may sign the uploaded ZIP. Ignore only signature and generated
        # packaging relationships/core metadata, preserving nuspec and all code.
        return {name: archive.read(name) for name in names
                if name != '.signature.p7s' and not name.startswith(('_rels/', 'package/services/'))}


def registry_package(version):
    check_version(version)
    package = PACKAGE.lower()
    request = urllib.request.Request(
        f'https://api.nuget.org/v3-flatcontainer/{package}/{version}/{package}.{version}.nupkg',
        headers={'User-Agent': 'codyps-zpl-release'},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def select_upload(source, version):
    data = source.read_bytes()
    local = payload(data, version)
    remote = registry_package(version)
    if remote is not None and payload(remote, version) != local:
        raise ValueError('Published NuGet version has different content; never overwrite or silently skip it')
    print(f'publish={"true" if remote is None else "false"}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('prepare')
    verify = commands.add_parser('verify')
    verify.add_argument('version')
    verify.add_argument('package', type=Path)
    verify.add_argument('--rid', choices=NATIVES, help='Allow a local single-platform development package')
    upload = commands.add_parser('select-upload')
    upload.add_argument('version')
    upload.add_argument('package', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if args.command == 'prepare':
        prepare(root)
    elif args.command == 'verify':
        payload(args.package.read_bytes(), args.version, [args.rid] if args.rid else None)
    else:
        select_upload(args.package, args.version)
