"""Select a current, conflict-free PR merge before starting the timing worker.

GitHub's test merge and nullable mergeability status are documented at:
https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request
"""
import json
import os
from pathlib import Path
import subprocess
import time


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True, timeout=60))


def select(event, repo):
    original = event['pull_request']
    for attempt in range(6):
        pr = api(f"repos/{repo}/pulls/{original['number']}")
        if (pr['state'] != 'open' or pr['head']['sha'] != original['head']['sha']
                or pr['base']['ref'] != original['base']['ref']):
            print('Skipping closed, superseded or retargeted PR')
            return {}
        if pr['mergeable'] is False:
            print('Skipping PR with merge conflicts')
            return {}
        if pr['mergeable'] is True and pr['merge_commit_sha']:
            merge = api(f"repos/{repo}/git/commits/{pr['merge_commit_sha']}")
            parents = [parent['sha'] for parent in merge['parents']]
            if parents == [pr['base']['sha'], pr['head']['sha']]:
                return dict(base=parents[0], head=pr['merge_commit_sha'], pr_head=parents[1])
        # A base update can briefly leave a stale test merge or unknown status.
        if attempt < 5:
            time.sleep(2)
    print('Skipping PR until GitHub confirms a merge of its current target and head')
    return {}


def main():
    if os.environ['GITHUB_EVENT_NAME'] == 'pull_request':
        revisions = select(json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text()),
                           os.environ['GITHUB_REPOSITORY'])
    else:
        revisions = dict(head=os.environ['GITHUB_SHA'])
    with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
        for key, value in revisions.items():
            stream.write(f'{key}={value}\n')
    if not revisions:
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
            stream.write('Rendering benchmarks skipped: no current, confirmed conflict-free PR merge.\n')


if __name__ == '__main__':
    main()
