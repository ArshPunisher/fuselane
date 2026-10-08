import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES } from '@fuselane/capture'
import { handOff, offerFor, type BrowserDownloads, type Item } from '../lib/handoff.ts'

const MB = 1024 * 1024
const big: Item = {
  id: 7,
  state: 'in_progress',
  url: 'https://cdn.example.org/big.iso',
  filename: '/Users/me/Downloads/big.iso',
  size: 700 * MB,
}

function fakeDownloads() {
  const calls: string[] = []
  const d: BrowserDownloads = {
    pause: async (id) => void calls.push(`pause ${id}`),
    resume: async (id) => void calls.push(`resume ${id}`),
    cancel: async (id) => void calls.push(`cancel ${id}`),
    erase: async (id) => void calls.push(`erase ${id}`),
  }
  return { d, calls }
}

test('accepted: the browser copy is cancelled and erased', async () => {
  const { d, calls } = fakeDownloads()
  const out = await handOff(big, DEFAULT_RULES, d, async () => ({
    v: 1,
    type: 'download.accepted',
    jobId: '3',
  }))
  assert.equal(out, 'handed-over')
  assert.deepEqual(calls, ['pause 7', 'cancel 7', 'erase 7'])
})

test('declined, broken, missing or slow app: the browser carries on', async () => {
  const cases: [string, () => Promise<unknown>][] = [
    [
      'declined',
      async () => ({ v: 1, type: 'download.declined', reason: 'unsupported', fallback: 'browser' }),
    ],
    ['garbage', async () => 'hello'],
    [
      'not installed',
      async () => Promise.reject(new Error('Specified native messaging host not found.')),
    ],
    ['too slow', () => new Promise(() => {})],
  ]
  for (const [why, ask] of cases) {
    const { d, calls } = fakeDownloads()
    const out = await handOff(big, DEFAULT_RULES, d, ask, 50)
    assert.equal(out, 'returned', why)
    assert.deepEqual(calls, ['pause 7', 'resume 7'], why)
  }
})

test('small, finished or skipped downloads are never touched', async () => {
  for (const it of [
    { ...big, size: 1000 },
    { ...big, state: 'complete' },
    { ...big, url: 'blob:https://x/1' },
  ]) {
    const { d, calls } = fakeDownloads()
    assert.equal(await handOff(it, DEFAULT_RULES, d, async () => ({})), 'kept')
    assert.deepEqual(calls, [])
  }
  const { d, calls } = fakeDownloads()
  assert.equal(
    await handOff(big, { ...DEFAULT_RULES, enabled: false }, d, async () => ({})),
    'kept',
  )
  assert.deepEqual(calls, [])
})

test('the offer carries the file name only, never the local folder', () => {
  const o = offerFor(
    { ...big, finalUrl: 'https://mirror.example.net/big.iso', referrer: 'https://example.org/' },
    'auto',
  )
  assert.equal(o.filename, 'big.iso')
  assert.equal(o.finalUrl, 'https://mirror.example.net/big.iso')
  assert.equal(o.cookies, null)
  assert.deepEqual(o.headers, {})
  assert.equal(
    offerFor({ ...big, finalUrl: big.url }, 'auto').finalUrl,
    null,
    'no redirect, no finalUrl',
  )
})

test('with permission, the site session goes with the offer; without it, nothing does', async () => {
  const offers: Record<string, unknown>[] = []
  const accept = async (o: unknown) => {
    offers.push(o as Record<string, unknown>)
    return { v: 1, type: 'download.accepted', jobId: '1' }
  }
  const asked: string[] = []
  const session = async (url: string) => {
    asked.push(url)
    return { cookies: 'sid=1; theme=dark', userAgent: 'Mozilla/5.0 (Test)' }
  }
  await handOff(big, DEFAULT_RULES, fakeDownloads().d, accept, 1000, session)
  assert.equal(offers[0]?.cookies, 'sid=1; theme=dark')
  assert.equal(offers[0]?.userAgent, 'Mozilla/5.0 (Test)')
  assert.deepEqual(asked, [big.url])

  // The final URL (after redirects) is the one whose cookies matter.
  await handOff(
    { ...big, finalUrl: 'https://mirror.example.org/big.iso' },
    DEFAULT_RULES,
    fakeDownloads().d,
    accept,
    1000,
    session,
  )
  assert.equal(asked[1], 'https://mirror.example.org/big.iso')

  // No permission, or reading cookies fails: the offer still goes, just without them.
  await handOff(big, DEFAULT_RULES, fakeDownloads().d, accept, 1000, async () => null)
  await handOff(big, DEFAULT_RULES, fakeDownloads().d, accept, 1000, async () => {
    throw new Error('no permission')
  })
  for (const o of offers.slice(2)) {
    assert.equal(o.cookies, null)
    assert.equal(o.userAgent, null)
  }
  assert.equal(offers.length, 4)
})
