import { defineConfig } from '@playwright/test'

// The extension in a real Chromium talking to Fuselane's native-messaging host
// (STEPS 7.4, 7.8). Needs `wxt build` and the `fuselane` CLI built first.
export default defineConfig({
  testDir: 'e2e',
  timeout: 60_000,
  retries: 0,
  reporter: [['list']],
})
