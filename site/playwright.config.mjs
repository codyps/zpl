import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  testMatch: '**/*.browser.mjs',
  use: { baseURL: 'http://127.0.0.1:4173/zpl/', browserName: 'chromium' },
  webServer: { command: 'node tests/server.mjs', url: 'http://127.0.0.1:4173/zpl/', reuseExistingServer: false },
});
