import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES, decide, onDomain, type Rules } from '../src/rules.ts'

const MB = 1024 * 1024
const rules = (r: Partial<Rules> = {}): Rules => ({ ...DEFAULT_RULES, ...r })
const big = { url: 'https://cdn.example.org/big.iso', filename: 'big.iso', size: 700 * MB }

test('large web downloads are taken by default', () => {
  assert.deepEqual(decide(big, rules()), { capture: true })
  assert.deepEqual(
    decide({ ...big, size: -1 }, rules()),
    { capture: true },
    'unknown size is taken',
  )
})

test('the browser keeps what Fuselane cannot fetch or was told to skip', () => {
  for (const url of [
    'blob:https://example.org/1-2-3',
    'data:text/plain,hi',
    'file:///tmp/a',
    'chrome://x',
    'not a url',
  ]) {
    assert.equal(decide({ ...big, url, finalUrl: undefined }, rules()).capture, false, url)
  }
  assert.equal(decide(big, rules({ enabled: false })).capture, false)
  assert.equal(decide(big, rules(), true).capture, false, 'Alt held')
  assert.equal(decide({ ...big, size: 200 * 1024 }, rules()).capture, false, 'small file')
})

test('the final url (after redirects) is what counts', () => {
  const r = rules({ excludeDomains: ['mirror.example.net'] })
  assert.equal(decide({ ...big, finalUrl: 'https://mirror.example.net/big.iso' }, r).capture, false)
})

test('domains match themselves and subdomains, never look-alikes', () => {
  assert.ok(onDomain('example.org', 'example.org'))
  assert.ok(onDomain('cdn.example.org', 'Example.ORG'))
  assert.ok(onDomain('cdn.example.org', '.example.org'))
  assert.ok(!onDomain('badexample.org', 'example.org'))
  assert.ok(!onDomain('example.org.evil.test', 'example.org'))
  assert.ok(!onDomain('example.org', ''))
  const only = rules({ includeDomains: ['example.org'] })
  assert.equal(decide(big, only).capture, true)
  assert.equal(decide({ ...big, url: 'https://other.test/big.iso' }, only).capture, false)
})

test('file types and MIME types narrow what is taken', () => {
  const isos = rules({ extensions: ['ISO', '.zip'] })
  assert.equal(decide(big, isos).capture, true)
  assert.equal(decide({ ...big, filename: 'movie.mkv' }, isos).capture, false)
  assert.equal(
    decide({ ...big, filename: undefined }, isos).capture,
    true,
    'falls back to the url path',
  )
  const videos = rules({ mimeTypes: ['video/*', 'application/zip'] })
  assert.equal(decide({ ...big, mime: 'video/mp4' }, videos).capture, true)
  assert.equal(decide({ ...big, mime: 'application/zip; charset=binary' }, videos).capture, true)
  assert.equal(decide({ ...big, mime: 'text/html' }, videos).capture, false)
  assert.equal(decide({ ...big, mime: undefined }, videos).capture, false)
})
