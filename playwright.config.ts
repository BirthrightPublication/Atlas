import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    viewport: { width: 1440, height: 1000 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: {
      executablePath: process.env.ATLAS_CHROMIUM_PATH || (existsSync('/usr/bin/chromium') ? '/usr/bin/chromium' : undefined),
      args: ['--no-sandbox'],
    },
  },
  webServer: [
    { command: 'bash scripts/test-host.sh', url: 'http://127.0.0.1:47832/api/health', reuseExistingServer: false, timeout: 30_000 },
    { command: 'npm run dev', url: 'http://127.0.0.1:1420', reuseExistingServer: false, timeout: 30_000 },
  ],
});
