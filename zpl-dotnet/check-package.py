#!/usr/bin/env python3
"""Test packed packages through SDK-style consumers with isolated caches."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
README_SKIP = '<!-- readme-test: skip -->'


def readme_blocks():
    """Yield (name, line, code) for each runnable ```csharp block of this
    package's README and the workspace README.

    A `<!-- readme-test: skip -->` line directly before a fence excludes it.
    Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
    """
    for name, path in [('zpl-dotnet-README', ROOT / 'README.md'), ('README', ROOT.parent / 'README.md')]:
        text = path.read_text(encoding='utf-8')
        for match in re.finditer(r'^```csharp[ \t]*\n(.*?)^```[ \t]*$', text, re.MULTILINE | re.DOTALL):
            before = text[:match.start()].rstrip('\n').rsplit('\n', 1)[-1].strip()
            if before != README_SKIP:
                yield name, text.count('\n', 0, match.start()) + 1, match.group(1)


def nuget_config(path, feed):
    # Exact package ID mapping outranks the wildcard. Dependencies may use
    # nuget.org, but a public Codyps.Zpl cannot mask a broken local package.
    # https://learn.microsoft.com/en-us/nuget/consume-packages/package-source-mapping
    config = ET.Element('configuration')
    sources = ET.SubElement(config, 'packageSources')
    ET.SubElement(sources, 'clear')
    ET.SubElement(sources, 'add', key='under-test', value=str(feed.resolve()))
    ET.SubElement(sources, 'add', key='nuget.org', value='https://api.nuget.org/v3/index.json')
    mapping = ET.SubElement(config, 'packageSourceMapping')
    ET.SubElement(mapping, 'clear')
    for source, pattern in [('under-test', 'Codyps.Zpl'), ('nuget.org', '*')]:
        ET.SubElement(ET.SubElement(mapping, 'packageSource', key=source), 'package', pattern=pattern)
    ET.ElementTree(config).write(path, encoding='unicode')


def smoke_project(work, version, framework):
    for name in ('Smoke.csproj', 'Program.cs'):
        shutil.copyfile(ROOT / 'Smoke' / name, work / name)
    fixture = ROOT.parent / 'zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf'
    shutil.copyfile(fixture, work / 'probe.ttf')
    project = work / 'Smoke.csproj'
    tree = ET.parse(project)
    for name, value in [('PackageVersion', version), ('TargetFramework', framework)]:
        element = tree.find(f'./PropertyGroup/{name}')
        if element is None:
            raise ValueError(f'Missing smoke {name}')
        element.attrib.clear()
        element.text = value
    tree.write(project, encoding='unicode')
    return project


def check(feed, version, rid, framework, build_only=False, container=None, docker_platform=None):
    legacy = framework in ('net472', 'net48')
    if legacy and rid != 'win-x64':
        raise ValueError('.NET Framework tests require win-x64')
    with tempfile.TemporaryDirectory(prefix='zpl-dotnet-consumer-') as directory:
        work = Path(directory)
        project = smoke_project(work, version, framework)
        config = work / 'NuGet.Config'
        nuget_config(config, feed)

        def dotnet(*command, expected_error=None):
            result = subprocess.run(['dotnet', *command], cwd=work, text=True,
                                    capture_output=expected_error is not None, check=expected_error is None)
            if expected_error is not None and (result.returncode == 0 or expected_error not in result.stdout + result.stderr):
                raise AssertionError(f'Expected build rejection containing {expected_error!r}: {result.stdout}{result.stderr}')

        def run_binary(binary):
            args = ['dotnet', str(binary), version, str(work / 'probe.ttf')]
            if container:
                args = ['docker', 'run', '--rm', '--platform', docker_platform,
                        '-v', f'{work}:{work}', '-w', str(work), container, *args]
            subprocess.run(args, cwd=work, check=True)

        restore = ['restore', str(project), '--configfile', str(config),
                   '--packages', str(work / 'packages'), '-p:NuGetAudit=false']
        dotnet(*restore)
        assets = json.loads((work / 'obj/project.assets.json').read_text())
        target = next(value for key, value in assets['targets'].items() if '/' not in key)
        package = next(value for key, value in target.items() if key.lower() == f'codyps.zpl/{version}')
        expected = 'net472' if legacy else ('net8.0' if framework == 'net8.0' else 'netstandard2.0')
        assert any(f'/{expected}/' in name for name in package['compile']), package
        if legacy:
            dotnet('build', str(project), '-c', 'Release', '--no-restore')
            binary = work / f'bin/Release/{framework}/Smoke.exe'
            assert (binary.parent / 'zpl_c.dll').is_file(), 'Framework native-copy target did not deploy DLL'
            if not build_only:
                subprocess.run([str(binary), version, str(work / 'probe.ttf')], cwd=work, check=True)
            dotnet('build', str(project), '-c', 'Release', '--no-restore', '-p:PlatformTarget=x86',
                   expected_error='Set PlatformTarget to x64')
            # Also verify native-copy targets through an intermediate library.
            tree = ET.parse(project)
            package_group = tree.find('ItemGroup')
            reference = package_group.find('PackageReference')
            package_group.remove(reference)
            reference.set('Version', version)
            ET.SubElement(package_group, 'ProjectReference', Include='Library/Library.csproj')
            ET.SubElement(package_group, 'Compile', Remove='Library/**')
            tree.write(project, encoding='unicode')
            library = work / 'Library'
            library.mkdir()
            lib_project = ET.Element('Project', Sdk='Microsoft.NET.Sdk')
            properties = ET.SubElement(lib_project, 'PropertyGroup')
            ET.SubElement(properties, 'TargetFramework').text = framework
            refs = ET.SubElement(lib_project, 'ItemGroup')
            refs.append(reference)
            ET.SubElement(refs, 'PackageReference', Include='Microsoft.NETFramework.ReferenceAssemblies',
                          Version='1.0.3', PrivateAssets='all')
            ET.ElementTree(lib_project).write(library / 'Library.csproj', encoding='unicode')
            shutil.rmtree(work / 'bin')
            shutil.rmtree(work / 'obj')
            dotnet(*restore)
            dotnet('build', str(project), '-c', 'Release', '--no-restore')
            assert (binary.parent / 'zpl_c.dll').is_file(), 'Transitive native-copy target did not deploy DLL'
            if not build_only:
                subprocess.run([str(binary), version, str(work / 'probe.ttf')], cwd=work, check=True)
            return

        dotnet('build', str(project), '-c', 'Release', '--no-restore')
        run_binary(work / f'bin/Release/{framework}/Smoke.dll')
        dotnet(*restore, '--runtime', rid, '-p:SelfContained=false', '-p:UseAppHost=false',
               '-p:EnableRuntimePackDownload=false')
        dotnet('publish', str(project), '-c', 'Release', '--no-restore',
               '--runtime', rid, '--self-contained', 'false', '-p:UseAppHost=false',
               '-p:EnableRuntimePackDownload=false', '--output', str(work / 'published'))
        run_binary(work / 'published/Smoke.dll')

        # README examples are top-level-statement programs built against the same
        # package. They live under work/ (for container mounts) only after the
        # smoke project's last build, whose default globs would otherwise include them.
        # https://learn.microsoft.com/en-us/dotnet/csharp/fundamentals/program-structure/top-level-statements
        blocks = list(readme_blocks())
        assert blocks, 'README has no C# examples'
        for name, line, code in blocks:
            readme = work / f'{name}-{line}'
            readme.mkdir()
            shutil.copyfile(project, readme / 'Readme.csproj')
            (readme / 'Program.cs').write_text(code, encoding='utf-8')
            readme_project = readme / 'Readme.csproj'
            dotnet('restore', str(readme_project), '--configfile', str(config),
                   '--packages', str(work / 'packages'), '-p:NuGetAudit=false')
            dotnet('build', str(readme_project), '-c', 'Release', '--no-restore')
            args = ['dotnet', str(readme / f'bin/Release/{framework}/Readme.dll')]
            if container:
                args = ['docker', 'run', '--rm', '--platform', docker_platform,
                        '-v', f'{work}:{work}', '-w', str(readme), container, *args]
            subprocess.run(args, cwd=readme, check=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('feed', type=Path)
    parser.add_argument('version')
    parser.add_argument('rid')
    parser.add_argument('--framework', choices=['net8.0', 'net6.0', 'net472', 'net48'], default='net8.0')
    parser.add_argument('--build-only', action='store_true', help='Cross-compile Framework consumers without running Windows executables')
    parser.add_argument('--container', help='Run binaries in this target-runtime Docker image')
    parser.add_argument('--docker-platform', help='Docker platform for the target runtime')
    args = parser.parse_args()
    if bool(args.container) != bool(args.docker_platform):
        parser.error('--container and --docker-platform must be supplied together')
    check(args.feed, args.version, args.rid, args.framework, args.build_only, args.container, args.docker_platform)
