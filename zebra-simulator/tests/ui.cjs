// Run with the site's Playwright installation; no additional npm lockfile.
// NODE_PATH=site/node_modules node zebra-simulator/tests/ui.cjs
const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const { mkdtemp, rm, mkdir } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const net = require('node:net');

(async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'zebra-simulator-ui-'));
    const binary = process.env.SIMULATOR_BIN || path.resolve(__dirname, '../../target/debug/zebra-simulator');
    const server = spawn(binary, ['--tcp-addr', '127.0.0.1:0', '--json-addr', '127.0.0.1:0', '--http-addr', '127.0.0.1:0', '--database', path.join(directory, 'printer.sqlite')]);
    let browser;
    try {
        const { host, tcpPort } = await new Promise((resolve, reject) => {
            const timer = setTimeout(() => reject(Error('Simulator startup timed out')), 10000);
            let output = '';
            server.on('error', reject);
            server.on('exit', code => { clearTimeout(timer); reject(Error(`Simulator exited: ${code}\n${output}`)); });
            server.stderr.on('data', chunk => {
                output += chunk;
                const match = output.match(/Printer TCP: 127\.0\.0\.1:(\d+);.*request log: (http:\/\/127\.0\.0\.1:\d+\/)/);
                if (match) { clearTimeout(timer); resolve({ host: match[2], tcpPort: Number(match[1]) }); }
            });
        });
        browser = await chromium.launch({ headless: true, args: ['--no-sandbox'], ...(process.env.CHROMIUM_EXECUTABLE ? { executablePath: process.env.CHROMIUM_EXECUTABLE } : {}) });
        const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
        const errors = []; page.on('pageerror', error => errors.push(error.message));
        await page.goto(host);
        await page.waitForFunction(() => document.getElementById('live').textContent.includes('Live'));
        assert.equal(await page.locator('#no-requests').isVisible(), true);
        await page.getByRole('button', { name: 'Submit print request' }).click();
        await page.waitForFunction(() => document.querySelectorAll('.request').length === 1);
        await page.locator('.request').first().click();
        await page.waitForFunction(() => document.querySelector('#previews img')?.naturalWidth === 400);
        assert.equal(await page.locator('#previews img').evaluate(img => img.naturalHeight), 200);
        assert.match(await page.locator('#data').textContent(), /Hello, Zebra!/);

        // Verify a real TCP request updates the already-open browser.
        await new Promise((resolve, reject) => {
            const socket = net.connect(tcpPort, '127.0.0.1', () => socket.end('! U1 getvar "device.unique_id"\r\n'));
            socket.on('error', reject); socket.on('data', () => {}); socket.on('close', resolve);
        });
        await page.waitForFunction(() => document.querySelectorAll('.request').length === 2);
        await page.locator('.request').first().click();
        await page.waitForFunction(() => document.getElementById('response').textContent.includes('SIM000000001'));
        assert.equal(await page.locator('#previews img').count(), 0);
        await page.locator('#filter').selectOption('label'); assert.equal(await page.locator('.request').count(), 1);
        await page.locator('#filter').selectOption('');
        await page.locator('#search').fill('sgd'); assert.equal(await page.locator('.request').count(), 1);
        await page.locator('#search').fill('');

        // Request payloads must display as text, including markup-like values.
        const hostile = '<img src=x onerror="window.injected=true">';
        const response = await page.request.post(`${host}pstprnt`, { data: '{}'+JSON.stringify({'device.location': hostile}) });
        assert.equal(response.status(), 422); // embedded quotes are not a classic-safe setting value
        await page.waitForFunction(() => document.querySelectorAll('.request').length === 3);
        await page.locator('.request').first().click();
        await page.waitForFunction(() => document.getElementById('data').textContent.includes('window.injected'));
        assert.equal(await page.locator('#data img').count(), 0);
        assert.equal(await page.evaluate(() => window.injected), undefined);
        await page.locator('.request').last().click();
        await page.waitForFunction(() => document.querySelector('#previews img')?.naturalWidth === 400);
        if (process.env.SIMULATOR_SCREENSHOT_DIR) {
            await mkdir(process.env.SIMULATOR_SCREENSHOT_DIR, { recursive: true });
            await page.screenshot({ path: path.join(process.env.SIMULATOR_SCREENSHOT_DIR, 'desktop.png'), fullPage: true });
        }
        await page.setViewportSize({ width: 390, height: 844 });
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), 'Mobile layout overflows');
        if (process.env.SIMULATOR_SCREENSHOT_DIR) await page.screenshot({ path: path.join(process.env.SIMULATOR_SCREENSHOT_DIR, 'mobile.png'), fullPage: true });
        await page.reload();
        await page.waitForFunction(() => document.querySelectorAll('.request').length === 3);
        assert.deepEqual(errors, []);
        console.log('Browser checks passed: TCP live updates, print form, previews, filters, text escaping, reload, and mobile layout.');
    } finally {
        if (browser) await browser.close();
        server.kill('SIGINT');
        await new Promise(resolve => { if (server.exitCode !== null) resolve(); else server.once('exit', resolve); });
        await rm(directory, { recursive: true, force: true });
    }
})().catch(error => { console.error(error); process.exitCode = 1; });
