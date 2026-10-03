"""Trusted main-only history publisher and shared measurement validation.

Artifacts are data, never executable input. PR records are validated for the
separate comment workflow; main() still rejects them for history publication.

workflow_run trust boundary:
https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run
"""
import argparse
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import tempfile

from common import NAMES, RUNNERS
from build import METHOD


def command(*args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def validate(record, run):
    labels = {'base', 'head'} if run['event'] == 'pull_request' else {'head'}
    commits = record.get('commits', {})
    if (record.get('schema') != 1 or record.get('run_id') != run['id']
            or record.get('run_attempt') != run['run_attempt']
            or record.get('event') != run['event']
            or record.get('runner') not in RUNNERS
            or set(commits) != labels or commits.get('head') != run['head_sha']
            or any(not isinstance(sha, str) or not re.fullmatch('[0-9a-f]{40}', sha)
                   for sha in commits.values())):
        raise ValueError('Unexpected benchmark identity')
    if run['event'] == 'pull_request' and (type(record.get('pr')) is not int or record['pr'] <= 0):
        raise ValueError('Invalid PR number')
    if (not isinstance(record.get('timestamp'), str)
            or not re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', record['timestamp'])):
        raise ValueError('Invalid timestamp')
    if not re.fullmatch('[0-9a-f]{64}', record.get('harness', '')):
        raise ValueError('Invalid harness')
    if not isinstance(record.get('environment'), dict) or not all(
            isinstance(record['environment'].get(key), str)
            for key in ('rust', 'os', 'cpu', 'image', 'rustflags', 'profile')):
        raise ValueError('Missing environment')
    # Schema 1 history remains readable. New provenance is additive, but must
    # be complete when supplied; never trust a producer's comparison verdict.
    if 'builds' in record or 'build_method' in record:
        builds = record.get('builds')
        if (record.get('build_method') != METHOD or not isinstance(builds, dict)
                or set(builds) != labels):
            raise ValueError('Invalid build provenance')
        for build in builds.values():
            if not isinstance(build, dict) or set(build) != {'inputs_sha256', 'binary_sha256'} or any(
                    not isinstance(value, str) or not re.fullmatch('[0-9a-f]{64}', value)
                    for value in build.values()):
                raise ValueError('Invalid build hashes')
    if set(record.get('samples', {})) != labels:
        raise ValueError('Missing benchmarks')
    for revision in labels:
        if set(record['samples'][revision]) != set(NAMES):
            raise ValueError('Missing benchmarks')
        for samples in record['samples'][revision].values():
            if not isinstance(samples, list) or len(samples) != 10 or any(
                    type(n) not in (int, float) or not math.isfinite(n) or not 0 < n < 1e12 for n in samples):
                raise ValueError('Invalid samples')
    return record


def archive(directory, records, run):
    data = directory / 'data'
    data.mkdir(exist_ok=True)
    for record, raw in records:
        key = f"{run['id']}-{run['run_attempt']}-{record['runner']}"
        # Immutable run/attempt keys make retrying publication idempotent.
        payload = json.dumps(record, sort_keys=True, indent=2) + '\n'
        target = data / f'{key}.json'
        if target.exists() and target.read_text() != payload:
            raise ValueError('Conflicting result for existing run')
        raw_target = data / f'{key}.txt'
        if raw_target.exists() and raw_target.read_text() != raw:
            raise ValueError('Conflicting raw evidence for existing run')
        target.write_text(payload)
        raw_target.write_text(raw)
    index = []
    for path in sorted(data.glob('*.json')):
        if path.name == 'index.json':
            continue
        record = json.loads(path.read_text())
        entry = {key: record[key] for key in ('run_id', 'run_attempt', 'runner', 'timestamp',
                                               'commits', 'environment', 'harness')}
        entry['file'] = path.name
        entry['medians'] = {name: statistics.median(values) for name, values in record['samples']['head'].items()}
        index.append(entry)
    index.sort(key=lambda r: (r['run_id'], r['run_attempt'], r['runner']))
    (data / 'index.json').write_text(json.dumps(index) + '\n')


def read_records(incoming, run):
    # download-artifact v8 extracts a single match directly into its destination;
    # multiple matches use named subdirectories. Accept both, but never duplicates.
    # https://github.com/actions/download-artifact
    sources = sorted(incoming.glob('result.json')) + sorted(incoming.glob('render-benchmark-*/result.json'))
    records = []
    for path in sources:
        raw = path.with_name('raw.txt')
        if path.parent.is_symlink() or any(
                p.is_symlink() or not p.is_file() or p.stat().st_size > 1_000_000 for p in (path, raw)):
            raise ValueError('Invalid artifact file')
        records.append((validate(json.loads(path.read_text()), run), raw.read_text()))
    if len(records) != len(RUNNERS) or {r['runner'] for r, _ in records} != RUNNERS:
        raise ValueError('Incomplete runner matrix')
    if len({r['harness'] for r, _ in records}) != 1:
        raise ValueError('Inconsistent harness')
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('incoming', type=Path)
    args = parser.parse_args()
    run = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())['workflow_run']
    repo = os.environ['GITHUB_REPOSITORY']
    if (run['conclusion'] != 'success' or run['head_branch'] != 'main'
            or run['head_repository']['full_name'] != repo
            or run['event'] not in ('push', 'schedule', 'workflow_dispatch')):
        raise ValueError('Only successful main measurements can enter history')
    live = json.loads(command('gh', 'api', f"repos/{repo}/actions/runs/{run['id']}"))
    if (live['run_attempt'] != run['run_attempt'] or live['conclusion'] != 'success'
            or live['path'] != '.github/workflows/benchmarks.yml'):
        raise ValueError('Unexpected or superseded workflow')
    records = read_records(args.incoming, run)
    # Fresh temporary repository contains only history data; never run its files.
    remote = command('git', 'remote', 'get-url', 'origin')
    with tempfile.TemporaryDirectory() as temp:
        directory = Path(temp)
        command('git', 'init', '-b', 'benchmarks', cwd=directory)
        command('git', 'remote', 'add', 'origin', remote, cwd=directory)
        # gh reads the short-lived GH_TOKEN from the environment, never a file.
        command('git', 'config', 'credential.helper', '!gh auth git-credential', cwd=directory)
        for _ in range(3):
            exists = command('git', 'ls-remote', '--heads', 'origin', 'benchmarks', cwd=directory)
            if exists:
                command('git', 'fetch', '--depth=1', 'origin', 'benchmarks', cwd=directory)
                command('git', 'reset', '--hard', 'FETCH_HEAD', cwd=directory)
            archive(directory, records, run)
            command('git', 'add', 'data', cwd=directory)
            if subprocess.run(['git', 'diff', '--cached', '--quiet'], cwd=directory).returncode == 0:
                return
            command('git', '-c', 'user.name=github-actions[bot]', '-c',
                    'user.email=41898282+github-actions[bot]@users.noreply.github.com',
                    'commit', '-m', f"chore(perf): record run {run['id']} attempt {run['run_attempt']}", cwd=directory)
            if subprocess.run(['git', 'push', 'origin', 'HEAD:refs/heads/benchmarks'], cwd=directory).returncode == 0:
                return
        raise RuntimeError('History push failed after three attempts')


if __name__ == '__main__':
    main()
