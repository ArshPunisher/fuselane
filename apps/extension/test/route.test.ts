import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES } from '@fuselane/capture'
import {
  DEFAULT_LIMIT,
  MAX_LIMIT,
  MIN_LIMIT,
  cleanLimit,
  route,
  type Situation,
} from '../lib/route.ts'

// Who downloads what (REVIEW-2026-10-11 D).

const iso = { url: 'https://releases.example.org/os.iso', size: 3_000_000_000 }
const small = { url: 'https://example.org/notes.pdf', size: 40_000 }
const base: Situation = {
  rules: DEFAULT_RULES,
  app: 'connected',
  siteAccess: true,
  limit: DEFAULT_LIMIT,
}

test('with the app installed, every download goes to it, big or small, open or closed', () => {
  assert.deepEqual(route(iso, base), { to: 'app' })
  assert.deepEqual(route(small, base), { to: 'app' })
  assert.deepEqual(route(small, { ...base, app: 'not-running' }), { to: 'app' })
})

test('the Fuselane switch off leaves everything to the browser', () => {
  const off = { ...base, rules: { ...DEFAULT_RULES, enabled: false } }
  assert.equal(route(iso, off).to, 'browser')
  assert.equal(route(iso, { ...off, app: 'not-installed' }).to, 'browser')
})

test('sites and types the person excluded stay in the browser', () => {
  const rules = { ...DEFAULT_RULES, excludeDomains: ['example.org'] }
  assert.equal(route(small, { ...base, rules }).to, 'browser')
})

test('without the app the extension downloads up to its limit; bigger files need a choice', () => {
  const noApp = { ...base, app: 'not-installed' as const }
  assert.deepEqual(route(small, noApp), { to: 'self' })
  assert.deepEqual(route(iso, noApp), { to: 'too-big', size: 3_000_000_000 })
  assert.deepEqual(route({ ...iso, size: 900_000_000 }, { ...noApp, limit: 1_000_000_000 }), {
    to: 'self',
  })
  // Unknown size: started, and stopped if it grows past the limit.
  assert.deepEqual(route({ url: iso.url, size: -1 }, noApp), { to: 'self' })
})

test('without site access the extension can not fetch files, so the browser does', () => {
  const r = route(small, { ...base, app: 'not-installed', siteAccess: false })
  assert.deepEqual(r, { to: 'browser', reason: 'no site access' })
})

test('the limit stays between 100 MB and 2 GB', () => {
  assert.equal(cleanLimit(5), MIN_LIMIT)
  assert.equal(cleanLimit(10 * MAX_LIMIT), MAX_LIMIT)
  assert.equal(cleanLimit(500_000_000), 500_000_000)
  assert.equal(cleanLimit('lots'), DEFAULT_LIMIT)
})
