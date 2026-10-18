#!/usr/bin/env python3
"""Run classic packages.config consumers against the packed Windows package."""
import argparse
import importlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

sdk = importlib.import_module('check-package')


def check(feed, version, framework):
    with tempfile.TemporaryDirectory(prefix='zpl-classic-consumer-') as directory:
        work = Path(directory)
        sdk.smoke_project(work, version, framework)
        config = work / 'NuGet.Config'
        sdk.nuget_config(config, feed)
        packages = work / 'packages'

        def nuget(*args):
            subprocess.run(['nuget', *args, '-ConfigFile', str(config), '-NonInteractive', '-NoCache'],
                           cwd=work, check=True)

        nuget('install', 'Codyps.Zpl', '-Version', version, '-Framework', framework,
              '-OutputDirectory', str(packages), '-DirectDownload')
        namespace = 'http://schemas.microsoft.com/developer/msbuild/2003'
        ET.register_namespace('', namespace)
        def element(parent, tag, text=None, **attrs):
            value = ET.SubElement(parent, '{' + namespace + '}' + tag, attrs)
            value.text = text
            return value
        project = ET.Element('{' + namespace + '}Project', ToolsVersion='15.0', DefaultTargets='Build')
        props = element(project, 'PropertyGroup')
        for name, value in {
            'Configuration': 'Release', 'OutputType': 'Exe', 'AssemblyName': 'Smoke',
            'TargetFrameworkVersion': 'v4.7.2' if framework == 'net472' else 'v4.8',
            'PlatformTarget': 'x64', 'LangVersion': '12.0', 'Nullable': 'enable',
            'AutoGenerateBindingRedirects': 'true', 'GenerateBindingRedirectsOutputType': 'true',
            'OutputPath': 'bin/Release/',
        }.items():
            element(props, name, value)
        items = element(project, 'ItemGroup')
        for name in ('System', 'System.Core'):
            element(items, 'Reference', Include=name)
        package_config = ET.Element('packages')
        # Simulate NuGet installation into a classic project, then delete the
        # installed files and restore the resulting packages.config from scratch.
        candidates = ['net48'] if framework == 'net48' else []
        candidates += ['net472', 'net471', 'net47', 'net462', 'net461', 'net46',
                       'net452', 'net451', 'net45', 'net40', 'netstandard2.0']
        for package in sorted(packages.iterdir()):
            nupkg = next(package.glob('*.nupkg'))
            with zipfile.ZipFile(nupkg) as archive:
                manifest = ET.fromstring(archive.read(next(n for n in archive.namelist() if n.endswith('.nuspec'))))
            for node in manifest.iter():
                node.tag = node.tag.rsplit('}', 1)[-1]
            metadata = manifest.find('metadata')
            ET.SubElement(package_config, 'package', id=metadata.findtext('id'),
                          version=metadata.findtext('version'), targetFramework=framework)
            lib = next((package / 'lib' / tfm for tfm in candidates if (package / 'lib' / tfm).is_dir()), None)
            if lib:
                for assembly in sorted(lib.glob('*.dll')):
                    reference = element(items, 'Reference', Include=assembly.stem)
                    element(reference, 'HintPath', str(assembly))
                    element(reference, 'Private', 'true')
        for name in ('Program.cs', 'GlobalUsings.cs'):
            element(items, 'Compile', Include=name)
        (work / 'GlobalUsings.cs').write_text(
            '\n'.join(f'global using {name};' for name in
                      ('System', 'System.IO', 'System.Linq', 'System.Collections.Generic', 'System.Threading.Tasks')))
        element(project, 'Import', Project='$(MSBuildToolsPath)/Microsoft.CSharp.targets')
        native_targets = packages / f'Codyps.Zpl.{version}/build/Codyps.Zpl.targets'
        element(project, 'Import', Project=str(native_targets), Condition=f"Exists('{native_targets}')")
        ET.ElementTree(project).write(work / 'Classic.csproj', encoding='unicode')
        ET.ElementTree(package_config).write(work / 'packages.config', encoding='unicode')
        shutil.rmtree(packages)
        nuget('restore', 'packages.config', '-PackagesDirectory', str(packages))
        assert native_targets.is_file(), 'Missing classic native-copy targets'
        subprocess.run(['msbuild', 'Classic.csproj', '/p:Configuration=Release', '/nologo', '/verbosity:minimal'],
                       cwd=work, check=True)
        binary = work / 'bin/Release/Smoke.exe'
        assert (binary.parent / 'zpl_c.dll').is_file(), 'Classic native-copy targets did not deploy DLL'
        subprocess.run([str(binary), version, str(work / 'probe.ttf')], cwd=work, check=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('feed', type=Path)
    parser.add_argument('version')
    parser.add_argument('--framework', choices=['net472', 'net48'], required=True)
    args = parser.parse_args()
    check(args.feed, args.version, args.framework)
