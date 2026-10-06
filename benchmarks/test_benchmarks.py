import copy
import json
from pathlib import Path
import tempfile
import unittest

from common import NAMES, compare
from build import METHOD
from publish import archive, read_records, validate

CONFIG = json.loads(Path(__file__).with_name('config.json').read_text())


def fixture():
    run = dict(id=12, run_attempt=1, head_sha='a' * 40, event='push')
    record = dict(schema=1, run_id=12, run_attempt=1, event='push', runner='ubuntu-24.04',
                  commits={'head': 'a' * 40}, harness='b' * 64, timestamp='2026-09-29T00:00:00Z',
                  build_method=METHOD,
                  builds={'head': dict(inputs_sha256='d' * 64, binary_sha256='e' * 64)},
                  environment={key: 'test' for key in ('rust', 'os', 'cpu', 'image', 'rustflags', 'profile')},
                  samples={'head': {name: [10000] * 10 for name in NAMES}})
    return record, run


class Benchmarks(unittest.TestCase):
    def test_legacy_history_and_strict_optional_build_provenance(self):
        record, run = fixture()
        legacy = copy.deepcopy(record)
        del legacy['builds'], legacy['build_method']
        validate(legacy, run)
        for mutation in (
            lambda r: r.pop('builds'), lambda r: r.pop('build_method'),
            lambda r: r.update(build_method='separate-paths'),
            lambda r: r.update(builds=[]), lambda r: r['builds'].pop('head'),
            lambda r: r['builds']['head'].update(inputs_sha256=True),
            lambda r: r['builds']['head'].update(binary_sha256='invalid'),
        ):
            candidate = copy.deepcopy(record)
            mutation(candidate)
            with self.subTest(record=candidate), self.assertRaises(ValueError):
                validate(candidate, run)

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

    def test_download_layouts_and_duplicate_rejection(self):
        record, run = fixture()
        for nested in (False, True):
            with self.subTest(nested=nested), tempfile.TemporaryDirectory() as temp:
                incoming = Path(temp)
                directory = incoming / 'render-benchmark-ubuntu-24.04' if nested else incoming
                directory.mkdir(exist_ok=True)
                (directory / 'result.json').write_text(json.dumps(record))
                (directory / 'raw.txt').write_text('raw evidence')
                self.assertEqual(read_records(incoming, run), [(record, 'raw evidence')])
                if nested:
                    (incoming / 'result.json').write_text(json.dumps(record))
                    (incoming / 'raw.txt').write_text('duplicate evidence')
                    with self.assertRaises(ValueError):
                        read_records(incoming, run)
                (directory / 'raw.txt').unlink()
                with self.assertRaises(ValueError):
                    read_records(incoming, run)
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(ValueError):
                read_records(Path(temp), run)

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
            self.assertEqual((root / 'data/12-1-ubuntu-24.04.txt').read_text(), 'raw one')
            record['samples']['head'][NAMES[0]][0] = 11000
            with self.assertRaises(ValueError):
                archive(root, [(record, 'raw one')], run)


if __name__ == '__main__':
    unittest.main()
