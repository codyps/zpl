"""Build an identical standalone Cargo harness against base/head, then alternate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

from build import METHOD, build_revision
from common import COMPARISON_NOTES, NAMES, compare, comparison_kind

HERE = Path(__file__).resolve().parent


def command(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def measure(binary, name, milliseconds):
    raw = command([str(binary), name, str(milliseconds)])
    count, nanos = map(int, raw.split())
    if count <= 0 or nanos <= 0:
        raise ValueError("Nonpositive measurement")
    return nanos / count, raw


def verify_pr_merge(base, head, pr_head):
    if base is None or not pr_head:
        raise ValueError('PR measurements require a target and PR head identity')
    base_sha = command(['git', 'rev-parse', 'HEAD'], cwd=base)
    parents = command(['git', 'show', '-s', '--format=%P', 'HEAD'], cwd=head).split()
    if parents != [base_sha, pr_head]:
        raise ValueError('Candidate must merge the measured target and PR head')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--head', type=Path, required=True)
    parser.add_argument('--base', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    pr_head = os.getenv('PR_HEAD_SHA', '')
    if os.getenv('GITHUB_EVENT_NAME') == 'pull_request':
        verify_pr_merge(args.base, args.head, pr_head)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    config = json.loads((HERE / 'config.json').read_text())
    harness = (HERE / 'harness.rs').read_bytes()
    protocol = hashlib.sha256()
    for name in ('harness.rs', 'run.py', 'build.py', 'common.py', 'config.json'):
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
                  build_method=METHOD, builds={},
                  environment=dict(rust=command(['rustc', '-Vv']), os=platform.platform(), cpu=cpu,
                                   image=os.getenv('ImageVersion', 'unknown'),
                                   rustflags=os.getenv('RUSTFLAGS', ''), profile='release'))
    if record['event'] == 'pull_request':
        record['pr_head_sha'] = pr_head
    binaries = {}
    for label, checkout in (('base', args.base), ('head', args.head)):
        if checkout is None:
            continue
        checkout = checkout.resolve()
        commit, binary, provenance = build_revision(
            checkout, output / 'build', output / 'builds' / label, harness)
        record['commits'][label] = commit
        record['builds'][label] = provenance
        print(f'{label}: {json.dumps(provenance)}', flush=True)
        binaries[label] = binary
        record['samples'][label] = {name: [] for name in NAMES}
    # Warm both retained executables after the final build, before paired rounds.
    for binary in binaries.values():
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
    report = summary(record)
    (output / 'summary.md').write_text(report)
    if os.getenv('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
            stream.write(report)


def summary(record):
    config = record['config']
    kind = comparison_kind(record) if 'base' in record['samples'] else None
    lines = ['## Rendering performance', '']
    if record.get('pr_head_sha'):
        lines += [f"Target `{record['commits']['base']}` → PR merge `{record['commits']['head']}` "
                  f"(PR head `{record['pr_head_sha']}`).", '']
    if kind in COMPARISON_NOTES:
        lines += [COMPARISON_NOTES[kind], '']
    candidate = 'PR merge' if record.get('pr_head_sha') else 'Head'
    lines += [f'| Case/stage | {candidate} ns/op | Observed paired timing change | Flag |',
             '| --- | ---: | ---: | --- |']
    for name in NAMES:
        value = statistics.median(record['samples']['head'][name])
        change = compare(record['samples']['base'][name], record['samples']['head'][name], config) if kind else None
        delta = f"{change['percent']:+.1f}%" if change else '—'
        flagged = kind == 'measured' and change['alert']
        lines.append(f"| {name} | {value:.0f} | {delta} | {'change' if flagged else '—'} |")
    lines += ['', 'Positive change means slower. Flags require ≥10%, ≥1 µs, and a 99% paired-bootstrap interval excluding zero. Informational only.']
    for label, build in record['builds'].items():
        lines += ['', f"{label} executable SHA-256: `{build['binary_sha256']}`; "
                  f"package inputs SHA-256: `{build['inputs_sha256']}`."]
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    main()
