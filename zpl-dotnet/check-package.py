#!/usr/bin/env python3
"""Restore, run and RID-publish a consumer from a local NuGet feed only."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('feed', type=Path)
    parser.add_argument('version')
    parser.add_argument('rid')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    with tempfile.TemporaryDirectory(prefix='zpl-dotnet-consumer-') as directory:
        work = Path(directory)
        # Fresh project and package cache prevent a source reference or stale
        # globally cached package from masking missing package assets.
        for name in ('Smoke.csproj', 'Program.cs'):
            shutil.copyfile(root / 'Smoke' / name, work / name)
        fixture = root.parent / 'zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf'
        shutil.copyfile(fixture, work / 'probe.ttf')
        project = work / 'Smoke.csproj'
        tree = ET.parse(project)
        version = tree.find('./PropertyGroup/PackageVersion')
        if version is None:
            raise ValueError('Missing smoke package version')
        version.attrib.clear()
        version.text = args.version
        tree.write(project, encoding='unicode')
        config = work / 'NuGet.Config'
        # --source below sets the feed; clear inherited user/repository sources.
        config.write_text('<configuration><packageSources><clear /></packageSources></configuration>')
        def dotnet(*command):
            subprocess.run(['dotnet', *command], cwd=work, check=True)
        dotnet('restore', str(project), '--configfile', str(config),
               '--source', str(args.feed.resolve()), '--packages', str(work / 'packages'),
               '-p:NuGetAudit=false')
        dotnet('run', '--project', str(project), '-c', 'Release', '--no-restore', '--', args.version, str(work / 'probe.ttf'))
        dotnet('restore', str(project), '--configfile', str(config),
               '--source', str(args.feed.resolve()), '--packages', str(work / 'packages'),
               '--runtime', args.rid, '-p:NuGetAudit=false',
               '-p:SelfContained=false', '-p:UseAppHost=false',
               # Framework-dependent publication needs no runtime packs. Avoid
               # the SDK prefetching unused packs from the isolated local feed.
               '-p:EnableRuntimePackDownload=false')
        dotnet('publish', str(project), '-c', 'Release', '--no-restore',
               '--runtime', args.rid, '--self-contained', 'false', '-p:UseAppHost=false',
               '-p:EnableRuntimePackDownload=false',
               '--output', str(work / 'published'))
        dotnet(str(work / 'published/Smoke.dll'), args.version, str(work / 'probe.ttf'))
