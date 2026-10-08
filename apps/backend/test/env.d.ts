// The bindings tests see (wrangler.jsonc, plus the migrations vitest.config.ts passes).
import type { D1Migration } from 'cloudflare:test'

declare global {
  namespace Cloudflare {
    interface Env {
      DB: D1Database
      UPLOADS: R2Bucket
      TEST_MIGRATIONS: D1Migration[]
      PUBLIC_URL: string
      R2_BUCKET: string
    }
  }
}
