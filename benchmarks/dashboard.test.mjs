import {test} from 'node:test';
import assert from 'node:assert/strict';
import {operations, series} from './dashboard.mjs';

test('environment and harness boundaries never join unlike measurements', () => {
  const base = {runner: 'linux', harness: 'a', environment: {rust: '1', cpu: 'x'}, medians: {'text/total': 42}, run_id: 1, run_attempt: 1};
  const data = [base, {...base, run_id: 2}, {...base, environment: {rust: '2', cpu: 'x'}}, {...base, harness: 'b'}, {...base, runner: 'mac'}];
  assert.deepEqual(series(data, 'linux', 'text/total').map(g => g.length), [2, 1, 1]);
  assert.equal(series(data, 'missing', 'text/total').length, 0);
});

test('new workloads appear without inventing samples in older history', () => {
  const old = {runner: 'linux', harness: 'old', environment: {}, medians: {'text/scene': 42}, run_id: 1, run_attempt: 1};
  const recent = {...old, run_id: 2, harness: 'new', medians: {'text/scene': 43, 'bitmap-text/scene': 64}};
  assert.deepEqual(operations([old, recent]), ['text/scene', 'bitmap-text/scene']);
  assert.deepEqual(series([old, recent], 'linux', 'bitmap-text/scene'), [[{record: recent, value: 64}]]);
});

test('package history keeps runtime versions and native-only records separate', () => {
  const old = {runner: 'linux', harness: 'old', environment: {rust: '1'}, medians: {'text/total': 42}, run_id: 1};
  const recent = {...old, run_id: 2, harness: 'packages', environment: {rust: '1', go: 'go1', node: 'v24'},
    medians: {'text/total': 43, 'go/dense/png': 64, 'node/initialize': 100}};
  const data = [old, recent, {...recent, run_id: 3, environment: {...recent.environment, go: 'go2'}},
    {...recent, run_id: 4, environment: {...recent.environment, node: 'v26'}}];
  assert.deepEqual(operations(data), ['text/total', 'go/dense/png', 'node/initialize']);
  assert.deepEqual(series(data, 'linux', 'go/dense/png').map(g => g.length), [1, 1, 1]);
});
