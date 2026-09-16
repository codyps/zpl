import { test, expect } from '@playwright/test';

test('real worker renders, downloads, navigates labels, and invalidates stale output', async ({ page }) => {
  const errors = [];
  const external = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', r => { if (!r.url().startsWith('http://127.0.0.1:4173/') && !r.url().startsWith('blob:')) external.push(r.url()); });
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toHaveText('Label 1 of 1. Rendered locally.');
  await expect(page.locator('#preview')).toBeVisible();
  expect(await page.locator('#preview').evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  const download = page.waitForEvent('download');
  await page.getByText('Download PNG').click();
  expect((await download).suggestedFilename()).toBe('label-1.png');
  await page.screenshot({ path: 'test-results/preview-desktop.png', fullPage: true });
  await page.locator('#source').fill('^XA^PW120^LL80^XZ^XA^FO10,10^GB20,20,2^FS^XZ');
  await expect(page.getByText('Download PNG')).toBeHidden();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toHaveText('Label 1 of 2. Rendered locally.');
  await page.locator('#label').fill('2');
  await page.locator('#label').press('Tab');
  await expect(page.getByRole('status')).toHaveText('Label 2 of 2. Rendered locally.');
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toContainText(/no labels?/i);
  await expect(page.locator('#preview')).toBeHidden();
  expect(errors).toEqual([]);
  expect(external).toEqual([]);
});

test('mobile layout stays within the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toContainText('Rendered locally.');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: 'test-results/preview-mobile.png', fullPage: true });
});

test('cancel and timeout leave the editor usable', async ({ page }) => {
  await page.route('**/worker.mjs', () => {});
  await page.clock.install();
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Rendering cancelled.');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await page.clock.fastForward(20_001);
  await expect(page.getByRole('status')).toContainText('exceeded 20 seconds');
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
});
