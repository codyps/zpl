"""Build and measure the shipped Go backend and the public Node Wasm package.

Both revisions use the same physical build path and identical external harnesses.
Go consumes its checked-in translation; Node builds its own Wasm/bindings with
the CLI version in that revision's lockfile. No timing comes from an action cache.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib
import zlib

from build import digest, encoded
from common import PACKAGE_CASES

HERE = Path(__file__).resolve().parent


def command(args, cwd=None, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True, timeout=600).strip()


def snapshot(checkout, build):
    sha = command(['git', 'rev-parse', 'HEAD'], cwd=checkout)
    if subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--'], cwd=checkout).returncode:
        raise ValueError('Benchmark checkouts must have no tracked edits')
    if build.exists():
        shutil.rmtree(build)
    source = build / 'source'
    source.mkdir(parents=True)
    with tempfile.TemporaryFile() as archive:
        subprocess.run(['git', 'archive', '--format=tar', sha], cwd=checkout, stdout=archive, check=True)
        archive.seek(0)
        with tarfile.open(fileobj=archive) as contents:
            contents.extractall(source, filter='data')
    return sha, source


def inventory(build, paths, graph):
    files = {}
    for path in sorted(set(paths)):
        path.resolve().relative_to(build)
        files[path.relative_to(build).as_posix()] = digest(path.read_bytes())
    return encoded(dict(files=files, graph=json.loads(json.dumps(graph).replace(str(build), '$BUILD'))))


def go_inputs(build, project, env):
    # Inventory the selected production files/embeds, not excluded backends or
    # tests. go list describes the actual build constraints and module graph.
    # https://pkg.go.dev/cmd/go#hdr-List_packages_or_modules
    text = command(['go', 'list', '-mod=readonly', '-buildvcs=false', '-deps', '-json', '.'], cwd=project, env=env)
    decoder, packages, paths = json.JSONDecoder(), [], [project / 'go.mod']
    while text.strip():
        package, end = decoder.raw_decode(text.lstrip())
        text = text.lstrip()[end:]
        if package.get('Standard'):
            continue
        directory = Path(package['Dir'])
        paths += [directory / name for key in ('GoFiles', 'EmbedFiles', 'CgoFiles') for name in package.get(key, [])]
        for name in ('go.mod', 'go.sum'):
            if (directory / name).exists():
                paths.append(directory / name)
        packages.append({key: package.get(key) for key in ('ImportPath', 'Imports', 'Module')})
    return inventory(build, paths, packages)


def node_inputs(build, source, env):
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--format-version=1',
                                  '--filter-platform', 'wasm32-unknown-unknown'], cwd=source, env=env))
    root = next(p['id'] for p in metadata['packages'] if p['name'] == 'zpl-wasm')
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    selected, pending = set(), [root]
    while pending:
        key = pending.pop()
        if key not in selected:
            selected.add(key)
            pending.extend(nodes[key]['dependencies'])
    packages = [p for p in metadata['packages'] if p['id'] in selected]
    paths = [source / 'Cargo.toml', source / 'Cargo.lock']
    for package in packages:
        if package['source'] is None:
            directory = Path(package['manifest_path']).parent
            paths += [p for p in directory.rglob('*') if p.is_file()]
    paths += [p for p in (source / 'zpl-node').iterdir() if p.is_file()]
    paths += [build / 'package-node.cjs']
    graph = dict(packages=packages, nodes=[nodes[key] for key in sorted(selected)])
    return inventory(build, paths, graph)


def bindgen(version, tools):
    # Tool sources are checksum-verified by Cargo. Reuse the compiler tool, not
    # any package build output, across the two independently built revisions.
    binary = tools / ('wasm-bindgen-' + version) / 'bin/wasm-bindgen'
    if not binary.exists():
        installed = shutil.which('wasm-bindgen')
        if installed and command([installed, '--version']) == 'wasm-bindgen ' + version:
            return Path(installed)
        subprocess.run(['cargo', 'install', '--locked', '--version', version, '--root',
                        str(binary.parents[1]), 'wasm-bindgen-cli'], check=True, timeout=900,
                       env=dict(os.environ, CARGO_TARGET_DIR=str(tools / 'target')))
    if command([str(binary), '--version']) != 'wasm-bindgen ' + version:
        raise ValueError('Unexpected wasm-bindgen CLI')
    return binary


def build_packages(checkout, build, saved, tools):
    sha, source = snapshot(checkout, build)
    saved.mkdir(parents=True)
    project = build / 'go-harness'
    project.mkdir()
    shutil.copyfile(HERE / 'package-go.go', project / 'main.go')
    version = tomllib.loads((source / 'Cargo.lock').read_text())
    version = next(p['version'] for p in version['package'] if p['name'] == 'wasm-bindgen')
    cli = bindgen(version, tools)
    (project / 'go.mod').write_text('module zpl-performance\n\ngo 1.25.0\n\n'
        'require github.com/codyps/zpl/zpl-go v0.0.0\n'
        'replace github.com/codyps/zpl/zpl-go => ../source/zpl-go\n')
    env = dict(os.environ, CGO_ENABLED='0', GOTOOLCHAIN='local', GOCACHE=str(build / 'go-cache'),
               CARGO_TARGET_DIR=str(build / 'target'), CARGO_INCREMENTAL='0')
    go_inventory = go_inputs(build, project, env)
    go_binary = saved / 'go-runner'
    subprocess.run(['go', 'build', '-mod=readonly', '-trimpath', '-buildvcs=false',
                    '-ldflags=-s -w', '-o', str(go_binary), '.'], cwd=project, env=env, check=True, timeout=600)
    shutil.copyfile(HERE / 'package-node.cjs', build / 'package-node.cjs')
    node_inventory = node_inputs(build, source, env)
    # Build through the package's own release script, including wrapper changes.
    node_env = dict(env, PATH=str(cli.parent) + os.pathsep + os.environ['PATH'])
    subprocess.run(['bash', 'scripts/build-node.sh'], cwd=source, env=node_env, check=True, timeout=600)
    node_package = saved / 'node-package'
    node_package.mkdir()
    for name in ('index.cjs', 'index.mjs', 'index.d.ts', 'package.json', 'LICENSE'):
        shutil.copyfile(source / 'zpl-node' / name, node_package / name)
    shutil.copytree(source / 'zpl-node/pkg', node_package / 'pkg')
    node_payload = {p.relative_to(node_package).as_posix(): digest(p.read_bytes())
                    for p in sorted(node_package.rglob('*')) if p.suffix in ('.js', '.cjs', '.wasm')}
    # The CommonJS entry is resolved via main/type. Non-runtime description,
    # TypeScript declarations and license changes cannot manufacture an alert.
    manifest = json.loads((node_package / 'package.json').read_text())
    node_payload['loader'] = {key: manifest.get(key) for key in ('main', 'type', 'exports')}
    shutil.copyfile(HERE / 'package-node.cjs', saved / 'node-runner.cjs')
    shutil.copyfile(project / 'main.go', saved / 'go-runner.go')
    shutil.copyfile(project / 'go.mod', saved / 'go.mod')
    shutil.copyfile(source / 'Cargo.lock', saved / 'Cargo.lock')
    provenance = {}
    for name, data, executable in [('go', go_inventory, digest(go_binary.read_bytes())),
                                    ('node', node_inventory, digest(encoded(node_payload)))]:
        (saved / (name + '-inputs.json')).write_bytes(data)
        provenance[name] = dict(inputs_sha256=digest(data), binary_sha256=executable)
    (saved / 'node-payload.json').write_bytes(encoded(node_payload))
    return sha, {'go': [str(go_binary)], 'node': [shutil.which('node'), str(saved / 'node-runner.cjs'),
                                                str(node_package)]}, provenance


def invocation(program, name, milliseconds, output):
    package, case, *stage = name.split('/')
    if case == 'initialize':
        return program + ['initialize']
    width, height = (400, 300) if case == 'dense' else (812, 600)
    return program + ['png', str(HERE / 'fixtures' / (case + '.zpl')), str(width), str(height),
                      str(milliseconds), str(output)]


def png_evidence(path, case):
    # Packed grayscale samples and byte-wise filters follow the PNG specification:
    # https://www.w3.org/TR/PNG/#6Colour-values and https://www.w3.org/TR/PNG/#9Filters
    data = path.read_bytes()
    if len(data) < 33 or data[:8] != b'\x89PNG\r\n\x1a\n' or data[12:16] != b'IHDR':
        raise ValueError('Invalid PNG output')
    width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', data[16:29])
    expected = (400, 300) if case == 'dense' else (812, 600)
    if (width, height) != expected or depth not in (1, 8) or color != 0 or compression or filtering or interlace:
        raise ValueError('Unexpected PNG layout')
    offset, compressed, ended = 8, bytearray(), False
    while offset < len(data):
        if offset + 12 > len(data):
            raise ValueError('Truncated PNG chunk')
        size = struct.unpack('>I', data[offset:offset+4])[0]
        end = offset + size + 12
        if end > len(data) or zlib.crc32(data[offset+4:end-4]) != struct.unpack('>I', data[end-4:end])[0]:
            raise ValueError('Invalid PNG chunk')
        kind = data[offset+4:offset+8]
        if kind == b'IDAT':
            compressed.extend(data[offset+8:offset+8+size])
        if kind == b'IEND':
            if size or end != len(data):
                raise ValueError('Invalid PNG end')
            ended = True
        offset = end
    if not ended:
        raise ValueError('Missing PNG end')
    pixels = zlib.decompress(compressed)
    stride = (width * depth + 7) // 8
    if len(pixels) != height * (stride + 1):
        raise ValueError('Invalid PNG pixels')
    previous, ink = [0] * stride, 0
    for y in range(height):
        row = list(pixels[y*(stride+1)+1:(y+1)*(stride+1)])
        filter_type = pixels[y*(stride+1)]
        if filter_type > 4:
            raise ValueError('Invalid PNG filter')
        for x in range(stride):
            a, b, c = row[x-1] if x else 0, previous[x], previous[x-1] if x else 0
            paeth = min((a, b, c), key=lambda v: abs(a+b-c-v))
            predictor = (0, a, b, (a+b)//2, paeth)[filter_type]
            row[x] = (row[x] + predictor) & 255
        ink += (sum(v < 128 for v in row) if depth == 8 else
                sum(not (row[x // 8] & (128 >> (x % 8))) for x in range(width)))
        previous = row
    if not 0 < ink < width*height:
        raise ValueError('Blank/solid benchmark output')
    return dict(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data), width=width, height=height, black_pixels=ink)


def measure(program, name, milliseconds, output):
    result = json.loads(command(invocation(program, name, milliseconds, output)))
    if any(type(result.get(key)) is not int or result[key] <= 0 for key in ('iterations', 'ns', 'checksum')):
        raise ValueError('Invalid package timing')
    if name.endswith('/initialize') and (result['iterations'] != 1 or result['checksum'] != 1):
        raise ValueError('Initialization must measure one fresh instance')
    return result['ns'] / result['iterations'], json.dumps(result, sort_keys=True)
