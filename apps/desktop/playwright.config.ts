import { defineConfig, devices } from '@playwright/test'

// A second checkout (a git worktree) sets its own port, so it never reuses another
// checkout's dev server and tests the wrong code.
const port = Number(process.env.FUSELANE_TEST_PORT ?? 5191)

// UI tests run the real React app against the demo backend (src/lib/demo.ts) in a
// browser. WebKit is the engine closest to the macOS and Linux webviews.
export default defineConfig({
  testDir: 'tests',
  timeout: 30_000,
  fullyParallel: true,
  retries: 0,
  reporter: [['list']],
  use: { baseURL: `http://localhost:${port}` },
  webServer: {
    command: `pnpm exec vite --port ${port} --strictPort`,
    url: `http://localhost:${port}`,
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
