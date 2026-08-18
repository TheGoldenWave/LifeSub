import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/specs',
  timeout: 30_000,
  use: { baseURL: 'http://127.0.0.1:1421', trace: 'retain-on-failure' },
  webServer: { command: 'npm run dev -- --host 127.0.0.1', port: 1421, reuseExistingServer: true },
})
