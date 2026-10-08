import { defineConfig, devices } from '@playwright/test'

// UI tests run the real React app against the demo backend (src/lib/demo.ts) in a
// browser. WebKit is the engine closest to the macOS and Linux webviews.
export default defineConfig({
  testDir: 'tests',
  timeout: 30_000,
  fullyParallel: true,
  retries: 0,
  reporter: [['list']],
  use: { baseURL: 'http://localhost:5191' },
  webServer: {
    command: 'pnpm exec vite --port 5191 --strictPort',
    url: 'http://localhost:5191',
    reuseExistingServer: !process.env.CI,
  },
  projects: [
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'], viewport: { width: 1440, height: 900 } },
    },
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } },
    },
  ],
})
