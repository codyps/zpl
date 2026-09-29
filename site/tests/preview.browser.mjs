import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { readPng } from './png-reader.mjs';

test('real worker renders, downloads, navigates labels, and invalidates stale output', async ({ page }) => {
  const errors = [];
  const external = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', r => { if (!r.url().startsWith('http://127.0.0.1:4173/') && !r.url().startsWith('blob:')) external.push(r.url()); });
  await page.goto('./');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save SVG' })).toBeDisabled();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toHaveText('Label 1 of 1. Rendered locally.');
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#warnings')).toBeHidden();
  expect(await page.locator('#preview').evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('label-1.png');
  const savedBytes = await readFile(await saved.path());
  // Native Save Image As saves the resource at the displayed img.src.
  const imageDownload = page.waitForEvent('download');
  await page.locator('#preview').evaluate(img => {
    const link = document.createElement('a');
    link.href = img.src;
    link.download = 'displayed-label.png';
    link.click();
  });
  const displayed = await imageDownload;
  expect(await readFile(await displayed.path())).toEqual(savedBytes);
  const metadata = readPng(savedBytes).text;
  expect(JSON.parse(metadata.ZPL)).toMatchObject({
    source: await page.locator('#source').inputValue(), label: 1, width: 812, height: 1218, dpi: 203,
  });
  expect(metadata.Software).toBe(`zpl ${JSON.parse(metadata.ZPL).version}`);
  const svgDownload = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save SVG' }).click();
  const savedSvg = await svgDownload;
  expect(savedSvg.suggestedFilename()).toBe('label-1.svg');
  const svgMetadata = await page.evaluate(text => {
    const xml = new DOMParser().parseFromString(text, 'image/svg+xml');
    if (xml.querySelector('parsererror')) throw new Error('Invalid SVG');
    return JSON.parse(xml.querySelector('#zpl-metadata').textContent);
  }, await readFile(await savedSvg.path(), 'utf8'));
  expect(svgMetadata).toEqual(JSON.parse(metadata.ZPL));
  await page.screenshot({ path: 'test-results/preview-desktop.png', fullPage: true });
  await page.locator('#source').fill('^XA^PW120^LL80^XZ^XA^FO10,10^GB20,20,2^FS^XZ');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save SVG' })).toBeDisabled();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toHaveText('Label 1 of 2. Rendered locally.');
  await page.locator('#label').fill('2');
  await page.locator('#label').press('Tab');
  await expect(page.getByRole('status')).toHaveText('Label 2 of 2. Rendered locally.');
  const secondDownload = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const second = await secondDownload;
  expect(second.suggestedFilename()).toBe('label-2.png');
  expect(JSON.parse(readPng(await readFile(await second.path())).text.ZPL)).toMatchObject({
    source: await page.locator('#source').inputValue(), label: 2,
  });
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

test('workspace fills tall windows and fits portrait and landscape labels', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1200 });
  await page.goto('./');
  for (const [width, height] of [[812, 1218], [1600, 400]]) {
    await page.locator('#width').fill(String(width));
    await page.locator('#height').fill(String(height));
    await page.getByRole('button', { name: 'Render preview' }).click();
    await expect(page.getByRole('status')).toContainText('Rendered locally.');
    await expect(page.locator('#preview')).toBeVisible();
    const layout = await page.evaluate(() => {
      const canvas = document.querySelector('#canvas');
      const image = document.querySelector('#preview');
      const box = canvas.getBoundingClientRect();
      const label = image.getBoundingClientRect();
      return {
        fits: label.top >= box.top && label.bottom <= box.bottom &&
          label.left >= box.left && label.right <= box.right,
        scrolls: canvas.scrollHeight > canvas.clientHeight || canvas.scrollWidth > canvas.clientWidth,
        footerBottom: document.querySelector('footer').getBoundingClientRect().bottom,
        workspaceBottom: document.querySelector('.workspace').getBoundingClientRect().bottom,
        nativeSize: [image.naturalWidth, image.naturalHeight],
      };
    });
    expect(layout.fits).toBe(true);
    expect(layout.scrolls).toBe(false);
    expect(layout.footerBottom).toBe(1200);
    expect(layout.workspaceBottom).toBeGreaterThan(1100);
    expect(layout.nativeSize).toEqual([width, height]);
  }
});


test('SVG metadata escapes markup and preserves arbitrary source text', async ({ page }) => {
  await page.goto('./');
  const result = await page.evaluate(async () => {
    const { withSvgMetadata } = await import('./svg-metadata.mjs');
    const source = '^XA\r\n日本語 & </metadata><script>alert(1)</script> ]]> \0 \ufffe \uffff ^XZ';
    const original = '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h10v10z"/></svg>';
    const svg = new TextDecoder().decode(withSvgMetadata(new TextEncoder().encode(original), { source, version: 'test' }));
    const xml = new DOMParser().parseFromString(svg, 'image/svg+xml');
    return {
      valid: !xml.querySelector('parsererror'),
      script: !!xml.querySelector('script'),
      roundTrip: JSON.parse(xml.querySelector('#zpl-metadata').textContent).source === source,
      unchanged: svg.replace(/<metadata[^>]*>.*?<\/metadata>/s, '') === original,
    };
  });
  expect(result).toEqual({ valid: true, script: false, roundTrip: true, unchanged: true });
});
