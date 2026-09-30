import {test, expect} from '@playwright/test';

test('performance lives under its own project-relative path', async ({page}) => {
  await page.route('**/perf/data/index.json', route => route.fulfill({json: []}));
  await page.goto('perf/');
  await expect(page).toHaveTitle('ZPL rendering performance');
  await expect(page.getByRole('status')).toContainText('No measurements published yet');
  await expect(page.getByRole('link', {name: 'Label preview'})).toHaveAttribute('href', '../');
});

test('performance charts measured data, switches stages, and links raw evidence', async ({page}) => {
  const record = {run_id: 1, run_attempt: 1, runner: 'warbler-linux', harness: 'a',
    environment: {rust: 'rustc test', cpu: 'test CPU', os: 'Linux', image: 'test'},
    timestamp: '2026-09-29T00:00:00Z', commits: {head: 'a'.repeat(40)},
    medians: {'text/scene': 10000, 'text/total': 20000}, file: '1-1-warbler-linux.json'};
  await page.route('**/perf/data/index.json', route => route.fulfill({json: [record]}));
  await page.goto('perf/');
  await expect(page.locator('tbody tr')).toHaveCount(1);
  await expect(page.locator('svg circle')).toHaveCount(1);
  await expect(page.locator('tbody')).toContainText('10.00');
  await page.getByLabel('Workload and stage').selectOption('text/total');
  await expect(page.locator('tbody')).toContainText('20.00');
  await expect(page.getByRole('link', {name: 'Raw', exact: true})).toHaveAttribute('href', 'data/1-1-warbler-linux.txt');
  await page.screenshot({path: 'test-results/perf-dashboard.png', fullPage: true});
});

test('performance fetch failures are visible', async ({page}) => {
  await page.route('**/perf/data/index.json', route => route.fulfill({status: 503}));
  await page.goto('perf/');
  await expect(page.getByRole('alert')).toContainText('Could not load performance history: HTTP 503');
});
