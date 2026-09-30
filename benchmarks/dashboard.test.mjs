import {test} from 'node:test';
import assert from 'node:assert/strict';
import {series} from './dashboard.mjs';

test('environment and harness boundaries never join unlike measurements', () => {
  const base = {runner: 'linux', harness: 'a', environment: {rust: '1', cpu: 'x'}, medians: {'text/total': 42}, run_id: 1, run_attempt: 1};
  const data = [base, {...base, run_id: 2}, {...base, environment: {rust: '2', cpu: 'x'}}, {...base, harness: 'b'}, {...base, runner: 'mac'}];
  assert.deepEqual(series(data, 'linux', 'text/total').map(g => g.length), [2, 1, 1]);
  assert.equal(series(data, 'missing', 'text/total').length, 0);
});
