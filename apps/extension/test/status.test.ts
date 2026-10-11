import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES } from '@fuselane/capture'
import { appStatus, browserId, readHandoff, withHandoff } from '../lib/status.ts'

// What the extension tells the app about itself, and the app's setting for where
// downloads go (BROWSER-EXTENSION.md §7).

test('each browser is named the way the app knows it', () => {
  const chrome =
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36'
  assert.equal(browserId(chrome), 'chrome')
  assert.equal(browserId(`${chrome} Edg/141.0.0.0`), 'edge')
  assert.equal(browserId(chrome, true), 'brave')
  assert.equal(browserId(`${chrome} Vivaldi/7.6`), 'vivaldi')
  assert.equal(
    browserId('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chromium/141.0 Safari/537.36'),
    'chromium',
  )
  assert.equal(
    browserId(
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:143.0) Gecko/20100101 Firefox/143.0',
    ),
    'firefox',
  )
})

test('only a well-formed setting from the app is taken', () => {
  assert.deepEqual(readHandoff({ enabled: true, minBytes: 2_000_000_000 }), {
    enabled: true,
    minBytes: 2_000_000_000,
  })
  assert.deepEqual(readHandoff({ enabled: false, minBytes: 0 }), { enabled: false, minBytes: 0 })
  for (const bad of [
    undefined,
    null,
    'on',
    { enabled: 'yes', minBytes: 1 },
    { enabled: true, minBytes: -1 },
    { enabled: true, minBytes: 1.5 },
    { enabled: true, minBytes: 2 ** 41 },
    { enabled: true },
  ])
    assert.equal(readHandoff(bad), undefined, JSON.stringify(bad))
})

test("the app's setting replaces the switch and size, never sites or types", () => {
  const ours = { ...DEFAULT_RULES, includeDomains: ['ubuntu.com'], extensions: ['iso'] }
  const r = withHandoff(ours, { enabled: false, minBytes: 3 * 1024 ** 3 })
  assert.equal(r.enabled, false)
  assert.equal(r.minBytes, 3 * 1024 ** 3)
  assert.deepEqual(r.includeDomains, ['ubuntu.com'])
  assert.deepEqual(r.extensions, ['iso'])
  assert.equal(withHandoff(ours, undefined), ours)
})

test('the ping says which browser and version, and brings the setting back', async () => {
  let sent: unknown
  const s = await appStatus(
    async (m) => {
      sent = m
      return { type: 'pong', app: { version: '0.3.0', handoff: { enabled: true, minBytes: 0 } } }
    },
    { browser: 'edge', version: '0.3.0' },
  )
  assert.deepEqual(sent, { type: 'ping', browser: 'edge', version: '0.3.0' })
  assert.deepEqual(s.handoff, { enabled: true, minBytes: 0 })
  // An older app sends no setting: the extension keeps its own.
  const old = await appStatus(async () => ({ type: 'pong', app: { version: '0.2.0' } }))
  assert.equal(old.handoff, undefined)
})

test('a change made while the app was closed wins over its older value', async () => {
  const { settle } = await import('../lib/status.ts')
  const mine = { ...DEFAULT_RULES, enabled: false }
  const connected = {
    state: 'connected' as const,
    text: '',
    handoff: { enabled: true, minBytes: 1024 * 1024 },
  }
  // The app takes the waiting change: ours is kept and nothing waits any more.
  const sent: unknown[] = []
  const took = await settle(connected, mine, true, async (m) => {
    sent.push(m)
    return { type: 'handoff', handoff: { enabled: false, minBytes: mine.minBytes } }
  })
  assert.deepEqual(sent, [{ v: 1, type: 'handoff.set', enabled: false, minBytes: mine.minBytes }])
  assert.equal(took.rules.enabled, false)
  assert.equal(took.pending, false)
  // The app doesn't answer the change: ours is still kept, and it keeps waiting.
  const stuck = await settle(connected, mine, true, async (m) =>
    (m as { type: string }).type === 'handoff.set' ? { error: 'no' } : {},
  )
  assert.equal(stuck.rules.enabled, false)
  assert.equal(stuck.pending, true)
  // Nothing waiting: the app's setting is followed.
  const follow = await settle(connected, mine, false, async () => ({}))
  assert.equal(follow.rules.enabled, true)
  // App closed: nothing changes, and a waiting change keeps waiting.
  const closed = await settle({ state: 'not-running', text: '' }, mine, true, async () => {
    throw new Error('not called')
  })
  assert.deepEqual(closed, { rules: mine, pending: true })
})

test("an upgrade keeps the person's earlier switch and size until the app has them", async () => {
  const { mustPush } = await import('../lib/status.ts')
  // Upgraded: rules saved by the old version, never synced. Ours goes first.
  assert.equal(mustPush({ rules: { enabled: false } }), true)
  // A new install has no rules yet: follow the app.
  assert.equal(mustPush({}), false)
  // Synced before: follow the app, unless a change is waiting.
  assert.equal(mustPush({ rules: {}, handoffSynced: true }), false)
  assert.equal(mustPush({ rules: {}, handoffSynced: true, handoffPending: true }), true)
})
