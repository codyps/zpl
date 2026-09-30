import copy
import json
from pathlib import Path
import tempfile
import unittest

from common import NAMES, compare
from publish import archive, validate

CONFIG = json.loads(Path(__file__).with_name('config.json').read_text())


def fixture():
    run = dict(id=12, run_attempt=1, head_sha='a' * 40, event='push')
    record = dict(schema=1, run_id=12, run_attempt=1, event='push', runner='warbler-linux',
                  commits={'head': 'a' * 40}, harness='b' * 64, timestamp='2026-09-29T00:00:00Z',
                  environment={key: 'test' for key in ('rust', 'os', 'cpu', 'image', 'rustflags', 'profile')},
                  samples={'head': {name: [10000] * 10 for name in NAMES}})
    return record, run


class Benchmarks(unittest.TestCase):
    def test_paired_thresholds_and_noise(self):
        self.assertFalse(compare([10000] * 10, [10000] * 10, CONFIG)['alert'])
        self.assertTrue(compare([10000] * 10, [12000] * 10, CONFIG)['alert'])
        self.assertTrue(compare([10000] * 10, [8000] * 10, CONFIG)['alert'])
        self.assertFalse(compare([100] * 10, [150] * 10, CONFIG)['alert'])
        self.assertFalse(compare([10000] * 10, [7000, 13000] * 5, CONFIG)['alert'])

    def test_reject_partial_nonfinite_and_wrong_run(self):
        record, run = fixture()
        validate(record, run)
        for key, bad in [('run_id', 13), ('commits', {'head': 'c' * 40}), ('runner', '../oops')]:
            candidate = copy.deepcopy(record)
            candidate[key] = bad
            with self.assertRaises(ValueError):
                validate(candidate, run)
        for bad in ([1] * 9, [float('nan')] * 10, [float('inf')] * 10, [True] * 10, [-1] * 10):
            candidate = copy.deepcopy(record)
            candidate['samples']['head'][NAMES[0]] = bad
            with self.assertRaises(ValueError):
                validate(candidate, run)

    def test_history_accumulates_and_is_idempotent(self):
        record, run = fixture()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive(root, [(record, 'raw one')], run)
            archive(root, [(record, 'raw one')], run)
            second = copy.deepcopy(record)
            second['run_id'] = 13
            archive(root, [(second, 'raw two')], dict(run, id=13))
            entries = json.loads((root / 'data/index.json').read_text())
            self.assertEqual([r['run_id'] for r in entries], [12, 13])
            self.assertEqual((root / 'data/12-1-warbler-linux.txt').read_text(), 'raw one')
            record['samples']['head'][NAMES[0]][0] = 11000
            with self.assertRaises(ValueError):
                archive(root, [(record, 'raw one')], run)


if __name__ == '__main__':
    unittest.main()
