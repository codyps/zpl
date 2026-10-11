import {test, expect} from '@playwright/test';

test('performance lives under its own project-relative path', async ({page}) => {
  await page.route('**/perf/data/index.json', route => route.fulfill({json: []}));
  await page.goto('perf/');
  await expect(page).toHaveTitle('ZPL rendering performance');
  await expect(page.getByRole('status')).toContainText('No measurements published yet');
  await expect(page.getByRole('link', {name: 'Label preview'})).toHaveAttribute('href', '../');
});

test('performance charts measured data, switches stages, and links raw evidence', async ({page}) => {
  const record = {run_id: 1, run_attempt: 1, runner: 'ubuntu-24.04', harness: 'a',
    environment: {rust: 'rustc test', cpu: 'test CPU', os: 'Linux', image: 'test'},
    timestamp: '2026-09-29T00:00:00Z', commits: {head: 'a'.repeat(40)},
    medians: {'text/scene': 10000, 'text/total': 20000}, file: '1-1-ubuntu-24.04.json'};
  await page.route('**/perf/data/index.json', route => route.fulfill({json: [record]}));
  await page.goto('perf/');
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('svg circle')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('10.00');
  await page.getByLabel('Workload and stage').selectOption('text/total');
  await expect(page.locator('tbody')).toContainText('20.00');
  await expect(page.getByRole('link', {name: 'Raw', exact: true})).toHaveAttribute('href', 'data/1-1-ubuntu-24.04.txt');
  await page.screenshot({path: 'test-results/perf-dashboard.png', fullPage: true});
});

test('performance fetch failures are visible', async ({page}) => {
  await page.route('**/perf/data/index.json', route => route.fulfill({status: 503}));
  await page.goto('perf/');
  await expect(page.getByRole('alert')).toContainText('Could not load performance history: HTTP 503');
});

test('bitmap workloads added after the first history record can be selected', async ({page}) => {
  const old = {run_id: 1, run_attempt: 1, runner: 'ubuntu-24.04', harness: 'old',
    environment: {rust: 'rustc test', cpu: 'test CPU', os: 'Linux', image: 'test'},
    timestamp: '2026-09-29T00:00:00Z', commits: {head: 'a'.repeat(40)},
    medians: {'text/scene': 10000}, file: '1-1-ubuntu-24.04.json'};
  const recent = {...old, run_id: 2, harness: 'new', file: '2-1-ubuntu-24.04.json',
    medians: {'text/scene': 11000, 'bitmap-text/scene': 30000}};
  await page.route('**/perf/data/index.json', route => route.fulfill({json: [old, recent]}));
  await page.goto('perf/');
  await page.getByLabel('Workload and stage').selectOption('bitmap-text/scene');
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('svg circle')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('30.00');
  await expect(page.getByRole('link', {name: 'Raw', exact: true})).toHaveAttribute('href', 'data/2-1-ubuntu-24.04.txt');
});

test('package operations show runtime versions without filling native-only history', async ({page}) => {
  const old = {run_id: 1, run_attempt: 1, runner: 'ubuntu-24.04', harness: 'native',
    environment: {rust: 'rustc test', cpu: 'test CPU', os: 'Linux', image: 'test'},
    timestamp: '2026-10-11T00:00:00Z', commits: {head: 'a'.repeat(40)},
    medians: {'text/total': 10000}, file: '1-1-ubuntu-24.04.json'};
  const recent = {...old, run_id: 2, harness: 'packages', file: '2-1-ubuntu-24.04.json',
    environment: {...old.environment, go: 'go version go1.27.2 linux/amd64', node: 'v24.0.0'},
    medians: {'text/total': 11000, 'go/dense/png': 1800000, 'node/dense/png': 900000, 'node/initialize': 6000000}};
  await page.route('**/perf/data/index.json', route => route.fulfill({json: [old, recent]}));
  await page.goto('perf/');
  await page.getByLabel('Workload and stage').selectOption('go/dense/png');
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('1800.00');
  await expect(page.locator('tbody')).toContainText('go1.27.2');
  await expect(page.locator('tbody')).toContainText('v24.0.0');
  await page.getByLabel('Workload and stage').selectOption('node/dense/png');
  await expect(page.locator('tbody')).toContainText('900.00');
  await page.getByLabel('Workload and stage').selectOption('node/initialize');
  await expect(page.locator('tbody')).toContainText('6000.00');
  await page.screenshot({path: 'test-results/perf-packages.png', fullPage: true});
});
