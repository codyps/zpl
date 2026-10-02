"""Build an identical standalone Cargo harness against base/head, then alternate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time
import tomllib

from common import NAMES, compare

HERE = Path(__file__).resolve().parent


def command(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def measure(binary, name, milliseconds):
    raw = command([str(binary), name, str(milliseconds)])
    count, nanos = map(int, raw.split())
    if count <= 0 or nanos <= 0:
        raise ValueError("Nonpositive measurement")
    return nanos / count, raw


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--head', type=Path, required=True)
    parser.add_argument('--base', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    config = json.loads((HERE / 'config.json').read_text())
    harness = (HERE / 'harness.rs').read_bytes()
    protocol = hashlib.sha256()
    for name in ('harness.rs', 'run.py', 'common.py', 'config.json'):
        protocol.update((HERE / name).read_bytes())
    cpu = next(
        (line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines()
         if line.startswith('model name')), platform.machine())
    record = dict(schema=1, run_id=int(os.getenv('GITHUB_RUN_ID', '0')),
                  run_attempt=int(os.getenv('GITHUB_RUN_ATTEMPT', '1')),
                  event=os.getenv('GITHUB_EVENT_NAME', 'local'),
                  pr=int(os.getenv('PR_NUMBER', '0')),
                  runner=os.getenv('BENCH_RUNNER', platform.system()),
                  timestamp=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                  harness=protocol.hexdigest(), config=config, commits={}, samples={},
                  environment=dict(rust=command(['rustc', '-Vv']), os=platform.platform(), cpu=cpu,
                                   image=os.getenv('ImageVersion', 'unknown'),
                                   rustflags=os.getenv('RUSTFLAGS', ''), profile='release'))
    binaries = {}
    for label, checkout in (('base', args.base), ('head', args.head)):
        if checkout is None:
            continue
        checkout = checkout.resolve()
        record['commits'][label] = command(['git', 'rev-parse', 'HEAD'], cwd=checkout)
        # Separate workspace avoids changing either revision, including old revisions
        # without this benchmark. Reuse its exact dependency resolution.
        build = output / label
        (build / 'src').mkdir(parents=True)
        (build / 'src/main.rs').write_bytes(harness)
        (build / 'Cargo.toml').write_text(
            '[package]\nname = "zpl-perf-harness"\nversion = "0.0.0"\nedition = "2021"\n'
            '[workspace]\n[dependencies]\nzpl = { path = ' + json.dumps(str(checkout / 'zpl')) + ' }\n')
        shutil.copyfile(checkout / 'Cargo.lock', build / 'Cargo.lock')
        # Prune the workspace lock and add the local harness, rejecting any new
        # registry resolution. The subsequent build enforces this lock.
        subprocess.run(['cargo', 'metadata', '--format-version=1'], cwd=build,
                       stdout=subprocess.DEVNULL, check=True)
        original = tomllib.loads((checkout / 'Cargo.lock').read_text())
        resolved = tomllib.loads((build / 'Cargo.lock').read_text())
        locked = {(p['name'], p['version'], p.get('source'), p.get('checksum')) for p in original['package']}
        if any((p['name'], p['version'], p.get('source'), p.get('checksum')) not in locked
               for p in resolved['package'] if p['name'] != 'zpl-perf-harness'):
            raise ValueError('Harness changed dependency versions')
        env = dict(os.environ, CARGO_TARGET_DIR=str(build / 'target'))
        subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=build, env=env, check=True)
        binary = build / 'target/release/zpl-perf-harness'
        binaries[label] = binary
        record['samples'][label] = {name: [] for name in NAMES}
        for name in NAMES:
            measure(binary, name, 25)  # validates nonblank native canvas, then warms up
    with (output / 'raw.txt').open('w') as log:
        for round_id in range(config['rounds']):
            print(f"Round {round_id + 1}/{config['rounds']}", flush=True)
            names = NAMES if round_id % 2 == 0 else NAMES[::-1]
            labels = list(binaries) if round_id % 2 == 0 else list(reversed(binaries))
            for name in names:
                for label in labels:
                    ns, raw = measure(binaries[label], name, config['milliseconds'])
                    record['samples'][label][name].append(ns)
                    log.write(f'{round_id} {label} {name} {raw}\n')
                    log.flush()
    (output / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
    lines = ['## Rendering performance', '', '| Case/stage | Head ns/op | Paired change | Flag |',
             '| --- | ---: | ---: | --- |']
    for name in NAMES:
        value = statistics.median(record['samples']['head'][name])
        change = compare(record['samples']['base'][name], record['samples']['head'][name], config) if args.base else None
        delta = f"{change['percent']:+.1f}%" if change else '—'
        lines.append(f"| {name} | {value:.0f} | {delta} | {'change' if change and change['alert'] else '—'} |")
    lines += ['', 'Positive change means slower. Flags require ≥10%, ≥1 µs, and a 99% paired-bootstrap interval excluding zero. Informational only.']
    report = '\n'.join(lines) + '\n'
    (output / 'summary.md').write_text(report)
    if os.getenv('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
            stream.write(report)


if __name__ == '__main__':
    main()
