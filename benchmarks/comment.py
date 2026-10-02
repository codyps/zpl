"""Publish a sticky PR comparison from the existing rendering benchmark data.

Only trusted default-branch code runs here. The workflow_run trust boundary:
https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run
"""
import argparse
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

from common import NAMES, compare
from publish import read_records

MARKER = '<!-- zpl-benchmarks -->'


def api(path, method='GET', body=None):
    args = ['gh', 'api', path, '--method', method]
    if body is not None:
        args += ['--input', '-']
    response = subprocess.check_output(args, text=True, timeout=60,
                                       input=json.dumps(body) if body is not None else None)
    return json.loads(response) if response.strip() else None


def comment_body(records, run, repo, config):
    lines = [MARKER, f"<!-- benchmark-run: {run['id']} attempt: {run['run_attempt']} -->",
             '## Rendering performance', '',
             f"Base `{records[0]['commits']['base'][:12]}` → head `{run['head_sha'][:12]}`.", '',
             'Both revisions use the same harness and compiler on a GitHub-hosted Linux worker, '
             'with ten alternating paired rounds.',
             f"Flags require ≥{config['threshold_percent']}% and ≥{config['threshold_ns'] / 1000:g} µs/op "
             'median paired change, with a 99% paired-bootstrap interval excluding zero. '
             'Positive means slower; negative means faster.', '']
    rows = []
    for record in records:
        for name in NAMES:
            before = record['samples']['base'][name]
            after = record['samples']['head'][name]
            change = compare(before, after, config)
            if change['alert']:
                low, high = change['interval']
                rows.append(f"| {name} | {statistics.median(before) / 1000:.3f} | "
                            f"{statistics.median(after) / 1000:.3f} | {change['percent']:+.1f}% | "
                            f"{low:+.1f}% to {high:+.1f}% |")
    if rows:
        lines += ['| Case/stage | Base µs/op | Head µs/op | Change | Interval |',
                  '| --- | ---: | ---: | ---: | ---: |', *rows]
    else:
        lines += ['No measured stage crosses the reporting thresholds in this run.']
    lines += ['', 'Informational only. Same-worker pairing reduces drift but does not eliminate noise; '
              'the interval is not a correction for multiple comparisons. PNG encoding is excluded.',
              f"[All measurements and raw samples](https://github.com/{repo}/actions/runs/{run['id']}) · "
              f"[Historical dashboard](https://{repo.split('/')[0]}.github.io/{repo.split('/')[1]}/perf/)"]
    return '\n'.join(lines), bool(rows)


def update_comment(records, run, repo, config):
    numbers = {record['pr'] for record in records}
    if len(numbers) != 1:
        raise ValueError('Inconsistent PR number')
    number = next(iter(numbers))
    associated = {pr['number'] for pr in run.get('pull_requests', [])}
    if associated and number not in associated:
        raise ValueError('PR is not associated with the source workflow')
    pr = api(f'repos/{repo}/pulls/{number}')
    # Fork runs can omit pull_requests. Validate live identity even when GitHub
    # does supply an association; artifact metadata alone never selects a PR.
    if (pr['state'] != 'open' or pr['base']['repo']['full_name'] != repo
            or pr['head']['sha'] != run['head_sha'] or not pr['head']['repo']
            or pr['head']['repo']['full_name'] != run['head_repository']['full_name']
            or pr['head']['ref'] != run['head_branch']
            or any(record['commits']['base'] != pr['base']['sha'] for record in records)):
        print('Skipping closed, obsolete or mismatched PR comparison')
        return
    previous = None
    page = 1
    while True:
        batch = api(f'repos/{repo}/issues/{number}/comments?per_page=100&page={page}')
        previous = previous or next((c for c in batch if c['user']['login'] == 'github-actions[bot]'
                                     and c['body'].startswith(MARKER)), None)
        if len(batch) < 100:
            break
        page += 1
    if previous:
        old = re.search(r'benchmark-run: (\d+) attempt: (\d+)', previous['body'])
        if old and (int(old[1]), int(old[2])) > (run['id'], run['run_attempt']):
            print('Skipping comparison older than the existing comment')
            return
    body, flagged = comment_body(records, run, repo, config)
    if previous:
        api(f"repos/{repo}/issues/comments/{previous['id']}", 'PATCH', {'body': body})
    elif flagged:
        api(f'repos/{repo}/issues/{number}/comments', 'POST', {'body': body})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('incoming', type=Path)
    args = parser.parse_args()
    event_run = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())['workflow_run']
    repo = os.environ['GITHUB_REPOSITORY']
    run = api(f"repos/{repo}/actions/runs/{event_run['id']}")
    if (run['run_attempt'] != event_run['run_attempt'] or run['conclusion'] != 'success'
            or run['event'] != 'pull_request' or run['path'] != '.github/workflows/benchmarks.yml'):
        raise ValueError('Unexpected, failed or superseded PR workflow')
    records = [record for record, _ in read_records(args.incoming, run)]
    config = json.loads(Path(__file__).with_name('config.json').read_text())
    # Thresholds are always from main. A PR cannot change its collection protocol
    # and still claim these samples used the trusted protocol.
    if any(any(record.get('config', {}).get(key) != config[key] for key in ('rounds', 'milliseconds'))
           for record in records):
        raise ValueError('Unexpected measurement protocol')
    update_comment(records, run, repo, config)


if __name__ == '__main__':
    main()
