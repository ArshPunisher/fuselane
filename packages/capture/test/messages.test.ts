import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { checkOffer, checkReply } from '../src/messages.ts'

// The same vectors run against the app's Rust validator.
const vectors = JSON.parse(
  readFileSync(new URL('../vectors/offers.json', import.meta.url), 'utf8'),
) as {
  valid: { why: string; offer: unknown }[]
  invalid: { why: string; offer: unknown }[]
}

test('every valid vector is accepted', () => {
  for (const v of vectors.valid) {
    const r = checkOffer(v.offer)
    assert.ok(r.ok, `${v.why}: ${r.ok ? '' : r.error}`)
  }
})

test('every hostile vector is refused with a reason', () => {
  assert.ok(vectors.invalid.length >= 25)
  for (const v of vectors.invalid) {
    const r = checkOffer(v.offer)
    assert.ok(!r.ok, `${v.why} was accepted`)
    assert.ok(r.error.length > 0)
  }
})

test('a checked offer fills defaults and keeps only allowed headers', () => {
  const r = checkOffer({
    v: 1,
    type: 'download.offer',
    url: 'https://example.org/a',
    headers: { 'X-Token': 'abc', Empty: null },
  })
  assert.ok(r.ok)
  assert.deepEqual(r.value.headers, { 'X-Token': 'abc' })
  assert.equal(r.value.source, 'auto')
  assert.equal(r.value.size, null)
})

test('only well-formed replies count; anything else means the browser keeps it', () => {
  assert.ok(checkReply({ v: 1, type: 'download.accepted', jobId: '42' }).ok)
  assert.ok(
    checkReply({ v: 1, type: 'download.declined', reason: 'unsupported', fallback: 'browser' }).ok,
  )
  for (const bad of [
    null,
    {},
    { v: 2, type: 'download.accepted', jobId: '1' },
    { v: 1, type: 'download.accepted', jobId: '' },
    { v: 1, type: 'download.declined', reason: 'because' },
    { v: 1, type: 'other' },
  ]) {
    assert.ok(!checkReply(bad).ok, JSON.stringify(bad))
  }
})
