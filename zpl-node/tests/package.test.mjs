import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const root = dirname(dirname(fileURLToPath(import.meta.url)));

test('packed archive installs and runs independently of the checkout', () => {
  const directory = mkdtempSync(join(tmpdir(), 'zpl-node-package-'));
  const env = { ...process.env, npm_config_cache: join(directory, 'cache') };
  // The nested test runner must start outside its parent's test context.
  delete env.NODE_TEST_CONTEXT;
  try {
    // Build first with npm run build. Avoid recursively executing lifecycle hooks.
    // https://docs.npmjs.com/cli/commands/npm-pack
    const [pack] = JSON.parse(execFileSync('npm', ['pack', '--ignore-scripts', '--json', '--pack-destination', directory], {
      cwd: root, env, encoding: 'utf8',
    }));
    assert.ok(pack.files.some(file => file.path === 'pkg/zpl_wasm_bg.wasm'));
    assert.ok(pack.files.some(file => file.path === 'LICENSE'));
    assert.ok(pack.files.every(file => /^(index\.(cjs|mjs|d\.ts)|package\.json|README\.md|LICENSE|pkg\/[^/]+\.(js|wasm))$/.test(file.path)));
    execFileSync('npm', ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', '--no-package-lock', join(directory, pack.filename)], {
      cwd: directory, env, stdio: 'pipe',
    });
    copyFileSync(join(root, 'tests/render.test.mjs'), join(directory, 'render.test.mjs'));
    copyFileSync(join(root, 'tests/fonts.test.mjs'), join(directory, 'fonts.test.mjs'));
    copyFileSync(join(root, 'tests/parse.test.mjs'), join(directory, 'parse.test.mjs'));
    env.ZPL_TEST_FONT = join(directory, 'probe.ttf');
    copyFileSync(join(root, '../zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf'), env.ZPL_TEST_FONT);
    execFileSync(process.execPath, ['--test', 'render.test.mjs', 'fonts.test.mjs', 'parse.test.mjs'], { cwd: directory, env, stdio: 'pipe' });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
