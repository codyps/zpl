"""Selection and checkout regressions for target-versus-merge measurements."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import revisions
from run import verify_pr_merge


def pull_request():
    return dict(number=7, state='open', mergeable=True, merge_commit_sha='d' * 40,
                base=dict(sha='c' * 40, ref='main'), head=dict(sha='a' * 40))


class Revisions(unittest.TestCase):
    def select(self, responses):
        with patch.object(revisions, 'api', side_effect=responses), \
                patch.object(revisions.time, 'sleep'):
            return revisions.select({'pull_request': pull_request()}, 'owner/repo')

    def test_current_target_can_advance_beyond_event(self):
        pr = pull_request()
        pr['base']['sha'] = 'b' * 40
        result = self.select([pr, {'parents': [pr['base'], pr['head']]}])
        self.assertEqual(result, dict(base='b' * 40, head='d' * 40, pr_head='a' * 40))

    def test_conflicted_closed_superseded_and_retargeted_skip(self):
        for mutate in (lambda p: p.update(mergeable=False), lambda p: p.update(state='closed'),
                       lambda p: p['head'].update(sha='e' * 40),
                       lambda p: p['base'].update(ref='release')):
            pr = pull_request()
            mutate(pr)
            self.assertEqual(self.select([pr]), {})

    def test_unknown_mergeability_waits_but_is_bounded(self):
        pr = pull_request()
        unknown = dict(pr, mergeable=None)
        self.assertEqual(self.select([unknown] * 6), {})
        self.assertTrue(self.select([unknown, pr, {'parents': [pr['base'], pr['head']]}]))

    def test_stale_or_wrong_merge_parents_never_run(self):
        pr = pull_request()
        for parents in ([pr['head']], [pr['head'], pr['base']],
                        [dict(sha='e' * 40), pr['head']]):
            self.assertEqual(self.select([pr, dict(parents=parents)] * 6), {})
        self.assertEqual(self.select([dict(pr, merge_commit_sha=None)] * 6), {})

    def test_workflow_outputs_skip_or_pin_revisions(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'event').write_text(json.dumps({'pull_request': pull_request()}))
            env = dict(GITHUB_OUTPUT=str(root / 'output'), GITHUB_STEP_SUMMARY=str(root / 'summary'),
                       GITHUB_EVENT_PATH=str(root / 'event'), GITHUB_EVENT_NAME='pull_request',
                       GITHUB_REPOSITORY='owner/repo', GITHUB_SHA='f' * 40)
            with patch.dict(os.environ, env), patch.object(revisions, 'select', return_value={}):
                revisions.main()
            self.assertEqual((root / 'output').read_text(), '')
            self.assertIn('skipped', (root / 'summary').read_text())
            for event in ('push', 'schedule', 'workflow_dispatch'):
                (root / 'output').write_text('')
                with patch.dict(os.environ, env | dict(GITHUB_EVENT_NAME=event)), \
                        patch.object(revisions, 'api') as api:
                    revisions.main()
                api.assert_not_called()
                self.assertEqual((root / 'output').read_text(), f"head={'f' * 40}\n")

    def test_checkout_contains_both_target_and_pr_changes(self):
        # Actual diverged Git histories: a plain head must fail verification,
        # while a two-parent merge retains changes unique to both branches.
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)

            def git(*args, cwd=root):
                return subprocess.check_output(['git', *args], cwd=cwd, text=True,
                                               stderr=subprocess.STDOUT).strip()

            git('init', '-q', '-b', 'main')
            git('config', 'user.name', 'Benchmark test')
            git('config', 'user.email', 'benchmark@example.invalid')
            git('commit', '--allow-empty', '-qm', 'test: initial')
            git('checkout', '-qb', 'pr')
            (root / 'pr-change').write_text('candidate')
            git('add', '.')
            git('commit', '-qm', 'test: PR change')
            head = git('rev-parse', 'HEAD')
            git('checkout', '-q', 'main')
            (root / 'target-change').write_text('target')
            git('add', '.')
            git('commit', '-qm', 'test: target change')
            baseline = root / 'baseline'
            git('clone', '-q', str(root), str(baseline))
            git('checkout', '-q', 'pr')
            with self.assertRaisesRegex(ValueError, 'Candidate must merge'):
                verify_pr_merge(baseline, root, head)
            git('checkout', '-q', 'main')
            git('merge', '--no-ff', '-qm', 'test: merge candidate', 'pr')
            verify_pr_merge(baseline, root, head)
            self.assertEqual(git('ls-tree', '--name-only', 'HEAD').splitlines(),
                             ['pr-change', 'target-change'])
            with self.assertRaisesRegex(ValueError, 'Candidate must merge'):
                verify_pr_merge(baseline, root, 'e' * 40)
            with self.assertRaisesRegex(ValueError, 'require a target'):
                verify_pr_merge(None, root, head)


if __name__ == '__main__':
    unittest.main()
