"""PR publishing regressions; all GitHub calls are mocked.

Trust boundary: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run
"""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import comment
from common import NAMES, compare
import publish
from test_benchmarks import CONFIG, fixture

REPO = 'codyps/zpl'


def comparison(factor=1.2):
    record, run = fixture()
    run.update(event='pull_request', conclusion='success', head_branch='speedup',
               head_repository={'full_name': 'contributor/zpl'}, path='.github/workflows/benchmarks.yml')
    record.update(event='pull_request', pr=7, config=copy.deepcopy(CONFIG))
    record['commits']['base'] = 'c' * 40
    record['samples'] = {revision: {name: [10000 * multiplier] * 10 for name in NAMES}
                         for revision, multiplier in (('base', 1), ('head', factor))}
    return record, run


def pull_request():
    return {'state': 'open', 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'contributor/zpl'},
                                    'ref': 'speedup'},
            'base': {'sha': 'c' * 40, 'repo': {'full_name': REPO}}}


def previous(run_id=11, attempt=1, author='github-actions[bot]'):
    return {'id': 42, 'user': {'login': author},
            'body': comment.MARKER + f'\n<!-- benchmark-run: {run_id} attempt: {attempt} -->'}


class Comments(unittest.TestCase):
    def fake_api(self, pr=None, comments=None):
        def respond(path, method='GET', body=None):
            if method != 'GET':
                return {}
            if '/pulls/' in path:
                return pull_request() if pr is None else pr
            return comments or []
        return patch.object(comment, 'api', side_effect=respond)

    def test_exact_threshold_in_both_directions(self):
        for after, percent in ((11000, 10), (9000, -10)):
            result = compare([10000] * 10, [after] * 10, CONFIG)
            self.assertTrue(result['alert'])
            self.assertEqual(result['percent'], percent)

    def test_nonfinite_nonpositive_and_boolean_timings(self):
        for value in (0, -1, True, float('nan'), float('inf')):
            with self.subTest(value=value), self.assertRaises(ValueError):
                compare([10000] * 10, [value] * 10, CONFIG)

    def test_comparison_validation_checks_both_revisions(self):
        record, run = comparison()
        publish.validate(record, run)
        mutations = (
            lambda r: r['samples'].pop('base'),
            lambda r: r['samples']['base'].pop(NAMES[0]),
            lambda r: r['samples']['base'].update({NAMES[0]: [float('nan')] * 10}),
            lambda r: r['commits'].update(base='invalid'),
            lambda r: r.update(pr=0), lambda r: r.update(pr=True),
            lambda r: r.update(run_attempt=2),
        )
        for mutation in mutations:
            item = copy.deepcopy(record)
            mutation(item)
            with self.subTest(record=item), self.assertRaises(ValueError):
                publish.validate(item, run)

    def test_improvement_and_regression_body(self):
        for factor, expected in ((0.8, '-20.0%'), (1.2, '+20.0%')):
            record, run = comparison(factor)
            body, flagged = comment.comment_body([record], run, REPO, CONFIG)
            self.assertTrue(flagged)
            self.assertIn(expected, body)
            self.assertIn('10.000', body)  # Stored ns/op are displayed as µs/op.
            self.assertIn('https://codyps.github.io/zpl/perf/', body)
            self.assertIn('actions/runs/12', body)

    def test_quiet_first_run_does_not_post(self):
        record, run = comparison(1)
        with self.fake_api() as api:
            comment.update_comment([record], run, REPO, CONFIG)
            self.assertTrue(all(len(call.args) == 1 for call in api.call_args_list))

    def test_post_first_alert_and_clear_old_alert(self):
        record, run = comparison()
        with self.fake_api() as api:
            comment.update_comment([record], run, REPO, CONFIG)
            self.assertEqual(api.call_args.args[1], 'POST')
        record, run = comparison(1)
        with self.fake_api(comments=[previous()]) as api:
            comment.update_comment([record], run, REPO, CONFIG)
            self.assertEqual(api.call_args.args[:2], (f'repos/{REPO}/issues/comments/42', 'PATCH'))
            self.assertIn('No measured stage', api.call_args.args[2]['body'])

    def test_ignores_human_marker_and_preserves_newer_run_or_attempt(self):
        record, run = comparison()
        with self.fake_api(comments=[previous(author='human')]) as api:
            comment.update_comment([record], run, REPO, CONFIG)
            self.assertEqual(api.call_args.args[1], 'POST')
        for old in (previous(run_id=13), previous(run_id=12, attempt=2)):
            with self.fake_api(comments=[old]) as api:
                comment.update_comment([record], run, REPO, CONFIG)
                self.assertTrue(all(len(call.args) == 1 for call in api.call_args_list))

    def test_finds_existing_comment_on_later_page(self):
        record, run = comparison()
        with patch.object(comment, 'api', side_effect=[pull_request(), [previous(author='human')] * 100,
                                                      [previous()], {}]) as api:
            comment.update_comment([record], run, REPO, CONFIG)
            self.assertIn('page=2', api.call_args_list[2].args[0])
            self.assertEqual(api.call_args.args[1], 'PATCH')

    def test_closed_stale_or_mismatched_pr_is_skipped(self):
        record, run = comparison()
        mutations = (
            lambda p: p.update(state='closed'), lambda p: p['head'].update(sha='e' * 40),
            lambda p: p['head'].update(ref='other'), lambda p: p['head'].update(repo=None),
            lambda p: p['head']['repo'].update(full_name='other/zpl'),
            lambda p: p['base'].update(sha='e' * 40),
            lambda p: p['base']['repo'].update(full_name='other/zpl'),
        )
        for mutation in mutations:
            pr = pull_request()
            mutation(pr)
            with self.subTest(pr=pr), self.fake_api(pr=pr) as api:
                comment.update_comment([record], run, REPO, CONFIG)
                self.assertEqual(api.call_count, 1)

    def test_known_workflow_association_cannot_select_other_pr(self):
        record, run = comparison()
        run['pull_requests'] = [{'number': 8}]
        with patch.object(comment, 'api') as api, self.assertRaisesRegex(ValueError, 'associated'):
            comment.update_comment([record], run, REPO, CONFIG)
        api.assert_not_called()

    def run_main(self, record, run, live=None):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            event = root / 'event.json'
            event.write_text(json.dumps({'workflow_run': run}))
            (root / 'result.json').write_text(json.dumps(record))
            (root / 'raw.txt').write_text('raw evidence')
            with patch.dict(os.environ, {'GITHUB_EVENT_PATH': str(event), 'GITHUB_REPOSITORY': REPO}), \
                    patch('sys.argv', ['comment.py', temp]), \
                    patch.object(comment, 'api', side_effect=[live or run, pull_request(), [], {}]) as api:
                comment.main()
                return api.call_args_list

    def test_complete_artifact_posts_using_trusted_thresholds(self):
        record, run = comparison()
        self.assertEqual(self.run_main(record, run)[-1].args[1], 'POST')
        record, run = comparison(1.05)
        record['config']['threshold_percent'] = 0
        calls = self.run_main(record, run)
        self.assertEqual(len(calls), 3)
        self.assertTrue(all(len(call.args) == 1 for call in calls))

    def test_failed_wrong_or_superseded_workflow_cannot_post(self):
        record, run = comparison()
        for change in ({'conclusion': 'failure'}, {'run_attempt': 2},
                       {'path': 'other.yml'}, {'event': 'push'}):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'PR workflow'):
                self.run_main(record, run, run | change)

    def test_changed_measurement_protocol_is_rejected(self):
        record, run = comparison()
        record['config']['milliseconds'] = 1
        with self.assertRaisesRegex(ValueError, 'protocol'):
            self.run_main(record, run)

    def test_history_still_rejects_pr_records(self):
        _, run = comparison()
        with tempfile.TemporaryDirectory() as temp:
            event = Path(temp) / 'event.json'
            event.write_text(json.dumps({'workflow_run': run}))
            with patch.dict(os.environ, {'GITHUB_EVENT_PATH': str(event), 'GITHUB_REPOSITORY': REPO}), \
                    patch('sys.argv', ['publish.py', temp]), patch.object(publish, 'command') as command, \
                    self.assertRaisesRegex(ValueError, 'Only successful main'):
                publish.main()
            command.assert_not_called()


if __name__ == '__main__':
    unittest.main()
