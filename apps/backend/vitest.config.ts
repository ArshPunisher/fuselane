import { defineConfig } from 'vitest/config'
import { cloudflareTest, readD1Migrations } from '@cloudflare/vitest-pool-workers'

// Tests run inside the real Workers runtime (workerd) with local D1 and R2.
export default defineConfig(async () => {
  const migrations = await readD1Migrations('migrations')
  return {
    plugins: [
      cloudflareTest({
        wrangler: { configPath: './wrangler.jsonc' },
        miniflare: { bindings: { TEST_MIGRATIONS: migrations } },
      }),
    ],
    test: { setupFiles: ['./test/apply-migrations.ts'] },
  }
})
