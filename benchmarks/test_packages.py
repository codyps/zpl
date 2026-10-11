"""Package-only changes must survive native Rust A/A suppression."""
import copy
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

from common import PACKAGE_CASES, PACKAGE_NAMES, PACKAGE_METHOD, comparison_kind
from comment import comment_body
from packages import go_inputs, inventory, invocation, measure, png_evidence
from publish import archive, read_records, validate
from run import summary, write_record
from test_comment import comparison, REPO
from test_benchmarks import CONFIG


def package_record():
    record, run = comparison(1)
    record.update(schema=2, package_method=PACKAGE_METHOD,
                  package_builds={label: {package: dict(inputs_sha256=inputs*64, binary_sha256=binary*64)
                                          for package in ('go', 'node')}
                                  for label, inputs, binary in [('base', '1', '2'), ('head', '3', '4')]},
                  outputs={})
    record['environment'].update(go='go version go1.27.2 linux/amd64', node='v24.0.0')
    record['builds']['head'] = copy.deepcopy(record['builds']['base'])
    for label in ('base', 'head'):
        record['samples'][label].update({name: [10000] * 10 for name in PACKAGE_NAMES})
        record['outputs'][label] = {}
        for case in PACKAGE_CASES:
            w, h = (400, 300) if case == 'dense' else (812, 600)
            for package in ('go', 'node'):
                record['outputs'][label][f'{package}/{case}/png'] = dict(sha256='5'*64, bytes=1024,
                                                                        width=w, height=h, black_pixels=50)
    return record, run


class PackageBenchmarks(unittest.TestCase):
    def test_package_only_changes_alert_even_when_native_executable_is_identical(self):
        for package in ('go', 'node'):
            record, run = package_record()
            name = package + '/dense/png'
            record['samples']['head'][name] = [12500] * 10
            other = 'node' if package == 'go' else 'go'
            record['package_builds']['head'][other] = copy.deepcopy(record['package_builds']['base'][other])
            validate(record, run)
            self.assertEqual(comparison_kind(record), 'unchanged')
            self.assertEqual(comparison_kind(record, name), 'measured')
            body, flagged = comment_body([record], run, REPO, CONFIG)
            self.assertTrue(flagged)
            self.assertIn('| ' + name + ' |', body)
            self.assertIn('+25.0%', body)
            self.assertIn('| ' + name + ' | 12500 | +25.0% | change |', summary(record))

    def test_each_package_has_independent_aa_and_nondeterminism_controls(self):
        for same, expected in [('both', 'unchanged'), ('binary_sha256', 'identical'),
                               ('inputs_sha256', 'unreproducible')]:
            record, run = package_record()
            for package in ('go', 'node'):
                for key in ('inputs_sha256', 'binary_sha256') if same == 'both' else (same,):
                    record['package_builds']['head'][package][key] = record['package_builds']['base'][package][key]
            for name in PACKAGE_NAMES:
                record['samples']['head'][name] = [14000] * 10
                self.assertEqual(comparison_kind(record, name), expected)
            self.assertFalse(comment_body([record], run, REPO, CONFIG)[1])
            self.assertNotIn('| change |', summary(record))

    def test_partial_or_untrusted_package_records_are_rejected(self):
        record, run = package_record()
        mutations = [lambda r: r.pop('package_builds'), lambda r: r.pop('package_method'),
                     lambda r: r.update(package_method='separate-paths'),
                     lambda r: r['package_builds']['base'].pop('go'),
                     lambda r: r['package_builds']['head']['node'].update(binary_sha256='bad'),
                     lambda r: r['samples']['head'].pop('go/dense/png'),
                     lambda r: r['samples']['base'].update({'node/initialize': [True]*10}),
                     lambda r: r['outputs']['head'].pop('go/dense/png'),
                     lambda r: r['outputs']['head']['node/dense/png'].update(sha256='6'*64),
                     lambda r: r['outputs']['base']['go/text/png'].update(black_pixels=0),
                     lambda r: r['outputs']['head']['go/text/png'].update(width=True),
                     lambda r: r['environment'].pop('go')]
        for mutation in mutations:
            candidate = copy.deepcopy(record)
            mutation(candidate)
            with self.subTest(record=candidate), self.assertRaises(ValueError):
                validate(candidate, run)

    def test_package_history_accumulates_without_rewriting_native_history(self):
        record, run = package_record()
        record['event'] = run['event'] = 'push'
        record['commits'].pop('base'); record['commits']['head'] = run['head_sha']
        for key in ('samples', 'builds', 'package_builds', 'outputs'):
            record[key].pop('base')
        validate(record, run)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive(root, [(record, 'all raw samples')], run)
            index = json.loads((root/'data/index.json').read_text())
            self.assertEqual(index[0]['medians']['go/dense/png'], 10000)
            self.assertEqual(index[0]['medians']['node/initialize'], 10000)

    def test_rollout_artifact_supports_old_and_new_trusted_publishers(self):
        record, run = package_record()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_record(root, record)
            (root/'raw.txt').write_text('all raw samples')
            legacy = json.loads((root/'result.json').read_text())
            self.assertEqual(legacy['schema'], 1)
            self.assertNotIn('package_builds', legacy)
            self.assertNotIn('go/dense/png', legacy['samples']['head'])
            validate(legacy, run)
            self.assertEqual(read_records(root, run), [(record, 'all raw samples')])
            (root/'result-packages.json').write_text('{}')
            with self.assertRaises(ValueError):
                read_records(root, run)

    def test_go_inventory_tracks_selected_helpers_embeds_and_wrapper_but_not_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            project = root/'go-harness'; project.mkdir()
            (project/'go.mod').write_text('module test')
            package = root/'source/zpl-go'; package.mkdir(parents=True)
            for name in ('zpl.go', 'module.go', 'module.dat', 'zpl_test.go', 'backend_wazero.go'):
                (package/name).write_text('original')
            graph = json.dumps(dict(Dir=str(package), ImportPath='package', GoFiles=['zpl.go', 'module.go'], EmbedFiles=['module.dat']))
            with patch('packages.command', return_value=graph):
                original = go_inputs(root, project, {})
                for name in ('zpl.go', 'module.go', 'module.dat'):
                    (package/name).write_text('changed')
                    self.assertNotEqual(go_inputs(root, project, {}), original)
                    (package/name).write_text('original')
                for name in ('zpl_test.go', 'backend_wazero.go'):
                    (package/name).write_text('changed')
                    self.assertEqual(go_inputs(root, project, {}), original)

    def test_fresh_initialization_and_invalid_measurements(self):
        self.assertEqual(invocation(['go-runner'], 'go/initialize', 200, 'unused'), ['go-runner', 'initialize'])
        with patch('packages.command', return_value=json.dumps(dict(iterations=1, ns=1000, checksum=1))):
            self.assertEqual(measure(['go'], 'go/initialize', 200, 'unused')[0], 1000)
        for data in [dict(iterations=2, ns=1000, checksum=1), dict(iterations=1, ns=True, checksum=1),
                     dict(iterations=1, ns=-1, checksum=1), dict(iterations=1, ns=1000, checksum=0)]:
            with patch('packages.command', return_value=json.dumps(data)), self.assertRaises(ValueError):
                measure(['go'], 'go/initialize', 200, 'unused')

    def test_png_evidence_rejects_blank_dimensions_and_invalid_pixels(self):
        def png(width=400, height=300, value=255, depth=8, mark=True):
            def chunk(kind, content):
                return struct.pack('>I', len(content)) + kind + content + struct.pack('>I', zlib.crc32(kind+content))
            pixels = bytearray((b'\0'+bytes([value])*((width*depth+7)//8))*height)
            if mark:
                pixels[1] = 0 if depth == 8 else 127
            return (b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR', struct.pack('>IIBBBBB', width,height,depth,0,0,0,0))
                    +chunk(b'IDAT',zlib.compress(pixels))+chunk(b'IEND',b''))
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/'output.png'
            path.write_bytes(png())
            self.assertEqual(png_evidence(path,'dense')['black_pixels'],1)
            path.write_bytes(png(width=812,height=600,depth=1))
            self.assertEqual(png_evidence(path,'text')['black_pixels'],1)
            for data in [png(width=100), png(value=0), png(mark=False), b'not PNG', png()[:-30], png()[:-12]]:
                path.write_bytes(data)
                with self.subTest(data=data[:30]), self.assertRaises((ValueError, struct.error, zlib.error)):
                    png_evidence(path,'dense')


if __name__ == '__main__':
    unittest.main()
