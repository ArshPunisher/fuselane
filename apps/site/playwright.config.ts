import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: 'tests',
  timeout: 30_000,
  retries: 0,
  reporter: [['list']],
  use: { baseURL: 'http://localhost:5196' },
  webServer: {
    command: 'pnpm exec vite --port 5196 --strictPort',
    url: 'http://localhost:5196',
    reuseExistingServer: !process.env.CI,
  },
  projects: [
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
})
