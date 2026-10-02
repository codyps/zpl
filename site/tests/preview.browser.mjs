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
  await expect(page.getByRole('button', { name: 'Save PDF' })).toBeDisabled();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 1');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#warnings')).toBeHidden();
  expect(await page.locator('#preview').evaluate(img => img.complete && img.naturalWidth > 0)).toBe(true);
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('print-1-label-1.png');
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
  expect(savedSvg.suggestedFilename()).toBe('print-1-label-1.svg');
  const svgMetadata = await page.evaluate(text => {
    const xml = new DOMParser().parseFromString(text, 'image/svg+xml');
    if (xml.querySelector('parsererror')) throw new Error('Invalid SVG');
    return JSON.parse(xml.querySelector('#zpl-metadata').textContent);
  }, await readFile(await savedSvg.path(), 'utf8'));
  expect(svgMetadata).toEqual(JSON.parse(metadata.ZPL));
  await page.screenshot({ path: 'test-results/preview-desktop.png', fullPage: true, animations: 'disabled' });
  await page.locator('#source').fill('^XA^PW120^LL80^XZ^XA^FO10,10^GB20,20,2^FS^XZ');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save SVG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save PDF' })).toBeDisabled();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 2 of 2');
  await expect(page.getByRole('status')).toBeEmpty();
  const secondDownload = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const second = await secondDownload;
  expect(second.suggestedFilename()).toBe('print-2-label-2.png');
  expect(JSON.parse(readPng(await readFile(await second.path())).text.ZPL)).toMatchObject({
    source: await page.locator('#source').inputValue(), label: 2,
  });
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toContainText(/no labels?/i);
  await expect(page.locator('#preview')).toBeHidden();
  await expect(page.locator('#error-paper')).toBeVisible();
  expect(errors).toEqual([]);
  expect(external).toEqual([]);
});

test('mobile layout stays within the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
    await expect(page.getByRole('status')).toBeEmpty();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: 'test-results/preview-mobile.png', fullPage: true, animations: 'disabled' });
});

test('cancel and timeout leave the editor usable', async ({ page }) => {
  await page.route('**/worker.mjs', () => {});
  await page.clock.install();
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.getByRole('status')).toBeEmpty();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await page.clock.fastForward(20_001);
  await expect(page.getByRole('status')).toContainText('exceeded 20 seconds');
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
});

test('workspace fills tall windows and fits portrait and landscape labels', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 1440, height: 1200 });
  await page.goto('./');
  for (const [width, height] of [[812, 1218], [1600, 400]]) {
    await page.locator('#width').fill(String(width));
    await page.locator('#height').fill(String(height));
    await page.getByRole('button', { name: 'Render preview' }).click();
    await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
    await expect(page.getByRole('status')).toBeEmpty();
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

test('history restores exact renders, source, settings and selected label within one tab', async ({ page, context }) => {
  await page.goto('./');
  const original = '^XA^PW120^LL80^XZ^XA^FO10,10^GB20,20,2^FS^XZ';
  await page.locator('#source').fill(original);
  await page.locator('#width').fill('200');
  await page.locator('#height').fill('150');
  await page.locator('#dpi').selectOption('300');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 2 of 2');
  const originalDownload = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const originalBytes = await readFile(await (await originalDownload).path());
  await page.locator('#source').fill('^XA^FO5,5^GB10,10,2^FS^XZ');
  await page.locator('#width').fill('400');
  await page.locator('#height').fill('300');
  await page.locator('#dpi').selectOption('203');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 1 of 1');
  await expect(page.locator('#history option')).toHaveCount(3);

  // Failed renders are not added, and an in-flight render can be replaced by history.
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toContainText(/no labels?/i);
  await expect(page.locator('#history option')).toHaveCount(3);
  await page.route('**/worker.mjs', () => {});
  await page.locator('#source').fill('^XA^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeEnabled();
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  await expect(page.locator('#source')).toHaveValue(original);
  await expect(page.locator('#width')).toHaveValue('200');
  await expect(page.locator('#height')).toHaveValue('150');
  await expect(page.locator('#dpi')).toHaveValue('300');
  await expect(page.locator('#label')).toHaveValue('2');
  await expect(page.locator('#dimensions')).toHaveText('120 × 80 dots');
  await expect(page.getByRole('status')).toBeEmpty();
  const restoredDownload = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  expect(await readFile(await (await restoredDownload).path())).toEqual(originalBytes);
  await expect(page.locator('#history option')).toHaveCount(3);
  await page.screenshot({ path: 'test-results/preview-history.png', fullPage: true });

  const otherTab = await context.newPage();
  await otherTab.goto('./');
  await expect(otherTab.getByLabel('Render history')).toBeDisabled();
  await otherTab.close();
  await page.getByRole('button', { name: 'Clear', exact: true }).click();
  await expect(page.getByLabel('Render history')).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
  await expect(page.locator('#source')).toHaveValue(original);
  await page.reload();
  await expect(page.getByLabel('Render history')).toBeDisabled();
});

test('history retains only the twenty most recent successful prints', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  await page.locator('#source').fill('^XA^PW10^LL10^XZ');
  for (let id = 1; id <= 21; id++) {
    await page.getByRole('button', { name: 'Render preview' }).click();
    await expect(page.getByLabel('Render history')).toHaveValue(String(id));
  }
  await expect(page.locator('#history option')).toHaveCount(21);
  await expect(page.locator('#history option[value="1"]')).toHaveCount(0);
  await page.getByLabel('Render history').selectOption('2');
  await expect(page.locator('#preview')).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('Render history')).toBeDisabled();
});

test('paper feeds downward briefly and reduced motion skips the animation', async ({ page }) => {
  await page.addInitScript(() => {
    window.printAnimations = [];
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const animation = animate.apply(this, args);
      if (this.id === 'preview' || this.id === 'previous-preview' || this.id === 'printer-slot') {
        queueMicrotask(() => animation.pause());
        window.printAnimations.push(animation);
      }
      return animation;
    };
  });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect.poll(() => page.evaluate(() => window.printAnimations.length)).toBe(2);
  const motion = await page.evaluate(() => {
    const image = document.querySelector('#preview');
    const animation = window.printAnimations[0];
    const bounds = [];
    for (const time of [80, 320]) {
      window.printAnimations.forEach(item => { item.currentTime = time; });
      const rect = image.getBoundingClientRect();
      bounds.push({ bottom: rect.bottom, top: rect.top, clip: getComputedStyle(image).clipPath });
    }
    return { duration: animation.effect.getTiming().duration, bounds };
  });
  expect(motion.duration).toBeLessThanOrEqual(500);
  expect(motion.bounds[1].bottom).toBeGreaterThan(motion.bounds[0].bottom);
  expect(motion.bounds[1].top).toBeGreaterThan(motion.bounds[0].top);
  expect(motion.bounds.every(bound => bound.clip === 'none')).toBe(true);
  await page.evaluate(() => window.printAnimations.forEach(animation => { animation.currentTime = 160; }));
  await page.screenshot({ path: 'test-results/preview-printing.png', fullPage: true });
  await page.locator('#source').fill('^XA^XZ');
  expect(await page.evaluate(() => document.querySelector('#preview').getAnimations().length)).toBe(0);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.evaluate(() => { window.printAnimations = []; });
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
    await expect(page.getByRole('status')).toBeEmpty();
  await page.locator('#preview').evaluate(img => img.decode());
  expect(await page.evaluate(() => window.printAnimations.length)).toBe(0);
});


test('successive sheets move through the full canvas with a gap and release the outgoing image', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const animation = animate.apply(this, args);
      if (['preview', 'previous-preview', 'printer-slot'].includes(this.id)) {
        queueMicrotask(() => animation.pause());
      }
      return animation;
    };
  });
  await page.goto('./');
  const firstSource = '^XA^FO20,20^A0N,50,50^FDFirst label^FS^XZ';
  await page.locator('#source').fill(firstSource);
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toBeVisible();
  await page.evaluate(() => document.querySelector('#preview').getAnimations().forEach(a => a.finish()));
  await page.locator('#source').fill('^XA^FO20,20^A0N,50,50^FDSecond label^FS^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#paper-feed')).toBeVisible();
  const movement = await page.evaluate(() => {
    const incoming = document.querySelector('#preview');
    const outgoing = document.querySelector('#previous-preview');
    const animations = document.querySelector('#canvas').getAnimations({ subtree: true });
    const samples = [];
    for (const time of [0, 100, 250, 419]) {
      animations.forEach(animation => { animation.currentTime = time; });
      const current = incoming.getBoundingClientRect();
      const previous = outgoing.getBoundingClientRect();
      samples.push({ seam: previous.top - current.bottom, outgoingTop: previous.top });
    }
    return { samples, clip: getComputedStyle(document.querySelector('#paper-feed')).overflow,
      viewport: getComputedStyle(document.querySelector('#canvas')).overflow,
      imageClip: getComputedStyle(incoming).clipPath,
      duration: animations.map(a => a.effect.getTiming().duration) };
  });
  expect(movement.clip).toBe('visible');
  expect(movement.viewport).toBe('hidden');
  expect(movement.imageClip).toBe('none');
  expect(movement.duration).toEqual([420, 420, 420]);
  for (const sample of movement.samples) expect(sample.seam).toBeGreaterThanOrEqual(15.9);
  expect(movement.samples.at(-1).outgoingTop).toBeGreaterThan(movement.samples[0].outgoingTop);
  const oldDownload = page.waitForEvent('download');
  await page.locator('#previous-preview').evaluate(img => {
    const link = document.createElement('a');
    link.href = img.src; link.download = 'old-label.png'; link.click();
  });
  expect(JSON.parse(readPng(await readFile(await (await oldDownload).path())).text.ZPL).source).toBe(firstSource);
  await page.evaluate(() => document.querySelector('#canvas').getAnimations({ subtree: true }).forEach(a => { a.currentTime = 100; }));
  await page.screenshot({ path: 'test-results/preview-paper-strip.png', fullPage: true });
  await page.evaluate(() => document.querySelector('#preview').getAnimations().forEach(a => a.finish()));
  await expect(page.locator('#paper-feed')).toBeHidden();
  await expect(page.locator('#previous-preview')).not.toHaveAttribute('src');
  await expect(page.locator('#preview')).toBeVisible();

  // A different label aspect ratio preserves the outgoing sheet without stretching its pixels.
  await page.locator('#source').fill('^XA^PW600^LL200^FO20,20^GB50,50,3^FS^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#paper-feed')).toBeVisible();
  expect(await page.locator('#previous-preview').evaluate(img => getComputedStyle(img).objectFit)).toBe('contain');
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.locator('#paper-feed')).toBeHidden();
  await expect(page.locator('#previous-preview')).not.toHaveAttribute('src');
  expect(await page.locator('#canvas').evaluate(el => el.getAnimations({ subtree: true }).length)).toBe(0);
  await expect(page.locator('#source')).toHaveValue(firstSource);
});

test('multi-label submissions feed in order and remain one navigable print', async ({ page }) => {
  let workerRequests = 0;
  page.on('request', request => { if (request.url().endsWith('/worker.mjs')) workerRequests++; });
  await page.addInitScript(() => {
    window.printedLabels = [];
    document.addEventListener('load', event => {
      if (event.target.id === 'preview') window.printedLabels.push(event.target.alt);
    }, true);
  });
  await page.goto('./');
  const source = '^XA^PW180^LL120^FO10,10^FDOne^FS^XZ\n^XA^PW200^LL140^FO10,10^FDTwo^FS^XZ\n^XA^PW220^LL160^FO10,10^FDThree^FS^XZ';
  await page.locator('#source').fill(source);
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 3 of 3');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => window.printedLabels)).toEqual([
    'Print #1, label 1 of 3', 'Print #1, label 2 of 3', 'Print #1, label 3 of 3',
  ]);
  await expect(page.locator('#history option')).toHaveCount(2);
  await expect(page.locator('#history option[value="1"]')).toContainText('Print #1 · 3 labels');
  await expect(page.locator('#print-id')).toHaveText('Print #1');
  await expect(page.locator('#label-total')).toHaveText('of 3');
  await expect(page.getByRole('button', { name: 'Next label' })).toBeDisabled();
  expect(workerRequests).toBe(3);
  for (const label of [2, 1]) {
    await page.getByRole('button', { name: 'Previous label' }).click();
    await expect(page.locator('#label')).toHaveValue(String(label));
    await expect(page.locator('#preview')).toHaveAttribute('alt', `Print #1, label ${label} of 3`);
    await expect(page.getByRole('status')).toBeEmpty();
  }
  expect(workerRequests).toBe(3);
  await expect(page.getByRole('button', { name: 'Previous label' })).toBeDisabled();
  await page.getByRole('button', { name: 'Next label' }).click();
  await expect(page.locator('#label')).toHaveValue('2');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PNG' }).click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('print-1-label-2.png');
  expect(JSON.parse(readPng(await readFile(await saved.path())).text.ZPL)).toMatchObject({
    print: 1, label: 2, labels: 3, source,
  });
  await page.screenshot({ path: 'test-results/preview-multi-print.png', fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: 'test-results/preview-multi-print-mobile.png', fullPage: true, animations: 'disabled' });

  // The same source submitted again is a new print; browsing cached pages is not.
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 3 of 3');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.locator('#history option')).toHaveCount(3);
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.locator('#print-id')).toHaveText('Print #1');
  await expect(page.locator('#label')).toHaveValue('2');
  await expect(page.locator('#source')).toHaveValue(source);
  expect(workerRequests).toBe(6);
});

test('cancelling a print stops its remaining labels without losing the completed label', async ({ page }) => {
  let workerRequests = 0;
  page.on('request', request => { if (request.url().endsWith('/worker.mjs')) workerRequests++; });
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (...args) {
      const animation = animate.apply(this, args);
      if (this.id === 'preview') queueMicrotask(() => animation.pause());
      return animation;
    };
  });
  await page.goto('./');
  await page.locator('#source').fill('^XA^PW100^LL80^XZ^XA^XZ^XA^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 3');
  await expect(page.getByRole('status')).toBeEmpty();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.locator('#history option')).toHaveCount(2);
  await page.getByRole('button', { name: 'Next label' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 2 of 3');
  await expect(page.getByRole('status')).toBeEmpty();
  expect(workerRequests).toBe(2);
  await expect(page.locator('#history option')).toHaveCount(2);
  await page.getByRole('button', { name: 'Clear', exact: true }).click();
  await expect(page.getByLabel('Render history')).toBeDisabled();
  await page.getByRole('button', { name: 'Previous label' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 3');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.getByLabel('Render history')).toBeDisabled();
});

test('a later label failure preserves the completed labels in their print', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  await page.locator('#source').fill('^XA^PW100^LL80^XZ^XA^PW5000^LL80^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('status')).toContainText('Print #1 · Label 2: ZPL label exceeds');
  await expect(page.locator('#history option')).toHaveCount(2);
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#label')).toHaveValue('1');
  await expect(page.locator('#label-total')).toHaveText('of 2');
  await expect(page.locator('#print-id')).toHaveText('Print #1');
});

test('the previous sheet stays visible while the next label is rendering', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  let requests = 0;
  await page.route('**/worker.mjs', route => {
    if (++requests === 1) return route.continue();
    // Keep the next worker pending to inspect the interval between labels.
  });
  await page.goto('./');
  await page.locator('#source').fill('^XA^PW120^LL80^XZ^XA^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect.poll(() => requests).toBe(2);
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 2');
  await expect(page.locator('#print-id')).toHaveText('Print #1');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
});

test('clicking Render keeps the old sheet visible through rendering and delayed image decoding', async ({ page }) => {
  await page.addInitScript(() => {
    const decode = HTMLImageElement.prototype.decode;
    window.decodeReleases = [];
    HTMLImageElement.prototype.decode = async function () {
      await decode.call(this);
      if (window.holdDecodes) await new Promise(resolve => window.decodeReleases.push(resolve));
    };
  });
  await page.goto('./');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  await expect(page.locator('#preview')).toBeVisible();
  const originalUrl = await page.locator('#preview').getAttribute('src');
  await page.evaluate(() => {
    window.holdDecodes = true;
    window.blankFrames = 0;
    window.watchFrames = true;
    function inspect() {
      const image = document.querySelector('#preview');
      const old = document.querySelector('#previous-preview');
      const visible = !image.hidden && getComputedStyle(image).visibility !== 'hidden' && image.complete && image.naturalWidth > 0;
      const outgoing = !document.querySelector('#paper-feed').hidden && old.complete && old.naturalWidth > 0;
      if (!visible && !outgoing) window.blankFrames++;
      if (window.watchFrames) requestAnimationFrame(inspect);
    }
    requestAnimationFrame(inspect);
  });
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect.poll(() => page.evaluate(() => window.decodeReleases.length)).toBe(2);
  await expect(page.locator('#preview')).toHaveAttribute('src', originalUrl);
  await expect(page.locator('#preview')).toBeVisible();
  await expect(page.locator('#placeholder')).toBeHidden();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await page.evaluate(() => {
    window.holdDecodes = false;
    window.decodeReleases.splice(0).forEach(resolve => resolve());
  });
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 1 of 1');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => { window.watchFrames = false; return window.blankFrames; })).toBe(0);
  await expect(page.locator('#preview')).not.toHaveAttribute('src', originalUrl);

  // Cancelling an image that is being prepared cannot replace the displayed sheet later.
  const currentUrl = await page.locator('#preview').getAttribute('src');
  await page.evaluate(() => { window.holdDecodes = true; });
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect.poll(() => page.evaluate(() => window.decodeReleases.length)).toBe(2);
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.evaluate(async () => {
    window.holdDecodes = false;
    window.decodeReleases.splice(0).forEach(resolve => resolve());
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  await expect(page.locator('#preview')).toHaveAttribute('src', currentUrl);
  await expect(page.getByRole('status')).toBeEmpty();
});

test('editing retains the preview and marks it stale until restored or rendered', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  const source = await page.locator('#source').inputValue();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  await expect(page.locator('#preview')).toBeVisible();
  const url = await page.locator('#preview').getAttribute('src');
  const bounds = await page.locator('#preview').boundingBox();
  await page.locator('#source').fill(`${source}\n`);
  await expect(page.locator('#preview')).toHaveAttribute('src', url);
  await expect(page.locator('#preview')).toBeVisible();
  expect(await page.locator('#preview').boundingBox()).toEqual(bounds);
  await expect(page.locator('#canvas')).toHaveClass(/is-stale/);
  await expect(page.locator('#preview-state')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save SVG' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Save PDF' })).toBeDisabled();
  await page.locator('#source').fill(source);
  await expect(page.locator('#preview-state')).toBeHidden();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
  await expect(page.locator('#preview')).toHaveAttribute('src', url);
  await page.locator('#width').fill('600');
  await expect(page.locator('#preview-state')).toBeVisible();
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 1 of 1');
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
  await expect(page.locator('#preview-state')).toBeHidden();
});

test('history and label navigation never animate paper feeding', async ({ page }) => {
  await page.addInitScript(() => {
    const animate = Element.prototype.animate;
    window.feedAnimations = 0;
    Element.prototype.animate = function (...args) {
      if (this.id === 'preview') window.feedAnimations++;
      return animate.apply(this, args);
    };
  });
  await page.goto('./');
  await page.locator('#source').fill('^XA^PW120^LL80^XZ^XA^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 2 of 2');
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => window.feedAnimations)).toBe(2);
  await page.getByRole('button', { name: 'Previous label' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 2');
  await page.getByRole('button', { name: 'Next label' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 2 of 2');
  await page.getByLabel('Render history').selectOption('1');
  await expect(page.locator('#preview')).toBeVisible();
  expect(await page.evaluate(() => window.feedAnimations)).toBe(2);
});

for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }]) {
  test(`history switches keep layout and paper visible at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('./');
    for (const source of ['^XA^PW120^LL80^XZ', '^XA^PW100^LL200^XZ^XA^XZ']) {
      await page.locator('#source').fill(source);
      await page.getByRole('button', { name: 'Render preview' }).click();
      await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
      await expect(page.getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
    }
    await page.evaluate(() => {
      const selectors = ['#canvas', '.preview-toolbar', '#previous-label', '#label', '#next-label', '.downloads', '#source', 'footer'];
      const measure = () => selectors.map(selector => {
        const { x, y, width, height } = document.querySelector(selector).getBoundingClientRect();
        return { x, y, width, height };
      });
      window.initialLayout = measure();
      window.layoutChanges = [];
      window.blankFrames = 0;
      window.watchSwitches = true;
      const check = () => {
        if (JSON.stringify(measure()) !== JSON.stringify(window.initialLayout)) window.layoutChanges.push(measure());
        const image = document.querySelector('#preview');
        if (image.hidden || !image.complete || !image.naturalWidth) window.blankFrames++;
        if (window.watchSwitches) requestAnimationFrame(check);
      };
      requestAnimationFrame(check);
      const decode = HTMLImageElement.prototype.decode;
      HTMLImageElement.prototype.decode = async function () {
        await decode.call(this);
        await new Promise(resolve => setTimeout(resolve, 100));
      };
    });
    for (const id of ['1', '2', '1']) {
      await page.getByLabel('Render history').selectOption(id);
      await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
      await expect(page.locator('#preview')).toHaveAttribute('alt', new RegExp(`Print #${id},`));
    }
    const result = await page.evaluate(() => {
      window.watchSwitches = false;
      return { changes: window.layoutChanges, blanks: window.blankFrames };
    });
    expect(result.changes).toEqual([]);
    expect(result.blanks).toBe(0);
  });
}

test('errors appear on animated handwritten paper and recover without layout shifts', async ({ page }) => {
  await page.goto('./');
  const canvas = await page.locator('#canvas').boundingBox();
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#error-paper')).toBeVisible();
  await expect(page.locator('#error-message')).toContainText(/no labels?/i);
  await page.evaluate(() => document.fonts.ready);
  expect(await page.locator('#error-paper').evaluate(el => getComputedStyle(el).fontFamily)).toContain('Patrick Hand');
  expect(await page.evaluate(() => document.fonts.check('24px "Patrick Hand"'))).toBe(true);
  expect(await page.locator('#canvas').boundingBox()).toEqual(canvas);
  await page.locator('#error-paper').evaluate(async el => { await Promise.all(el.getAnimations().map(a => a.finished)); });
  await page.screenshot({ path: 'test-results/preview-error-paper.png', fullPage: true });
  await page.locator('#source').fill('^XA^XZ');
  await page.locator('#width').fill('600');
  await expect(page.locator('#error-paper')).toBeVisible();
  await expect(page.locator('#error-message')).toContainText(/no labels?/i);
  expect(await page.locator('#error-paper').evaluate(el => el.getAnimations().length)).toBe(0);
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeEnabled();
  await expect(page.getByRole('status')).toBeEmpty();
  await expect(page.locator('#error-paper')).toBeHidden();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#error-paper')).toBeVisible();
  expect(await page.locator('#error-paper').evaluate(el => el.getAnimations().length)).toBe(0);
  await expect(page.getByRole('button', { name: 'Save PNG' })).toBeDisabled();
});


test('PDF downloads follow label selection, edits, undo, errors, and history', async ({ page }) => {
  // Inspect Blob MIME types without fetching blob: URLs, which the page CSP blocks.
  await page.addInitScript(() => {
    window.pdfBlobTypes = new Map();
    const create = URL.createObjectURL;
    URL.createObjectURL = blob => {
      const url = create(blob);
      window.pdfBlobTypes.set(url, blob.type);
      return url;
    };
  });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  const save = page.getByRole('button', { name: 'Save PDF' });
  await expect(save).toBeDisabled();
  const source = '^XA^PW203^LL406^XZ^XA^PW406^LL203^FO10,10^GB20,20,20^FS^XZ';
  await page.locator('#source').fill(source);
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 2 of 2');
  const downloadPdf = async (filename, size) => {
    const downloading = page.waitForEvent('download');
    await save.click();
    const download = await downloading;
    expect(download.suggestedFilename()).toBe(filename);
    const bytes = await readFile(await download.path());
    const pdf = bytes.toString('latin1');
    expect(pdf.startsWith('%PDF-1.7')).toBe(true);
    expect(pdf).toContain('/Count 1 ');
    expect(pdf.match(/\/MediaBox \[([^\]]+)\]/)[1].split(' ').map(Number)).toEqual([0, 0, ...size]);
    expect(await page.locator('#pdf').evaluate(a => window.pdfBlobTypes.get(a.href))).toBe('application/pdf');
    return bytes;
  };
  const second = await downloadPdf('print-1-label-2.pdf', [144, 72]);
  await page.locator('#previous-label').click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 2');
  const first = await downloadPdf('print-1-label-1.pdf', [72, 144]);
  expect(first).not.toEqual(second);
  await page.locator('#source').fill(source + '\n');
  await expect(save).toBeDisabled();
  await page.locator('#source').fill(source);
  await expect(save).toBeEnabled();
  expect(await downloadPdf('print-1-label-1.pdf', [72, 144])).toEqual(first);
  await page.locator('#source').fill('^XA^PW100^LL100^XZ');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #2, label 1 of 1');
  await expect(save).toBeEnabled();
  await page.locator('#history').selectOption('1');
  await expect(page.locator('#preview')).toHaveAttribute('alt', 'Print #1, label 1 of 2');
  expect(await downloadPdf('print-1-label-1.pdf', [72, 144])).toEqual(first);
  await page.locator('#source').fill('');
  await page.getByRole('button', { name: 'Render preview' }).click();
  await expect(page.locator('#error-paper')).toBeVisible();
  await expect(save).toBeDisabled();
});
