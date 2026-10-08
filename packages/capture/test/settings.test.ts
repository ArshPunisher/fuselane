import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES } from '../src/rules.ts'
import {
  MAX_MIN_BYTES,
  formatSize,
  normalizeRules,
  parseDomains,
  parseExtensions,
  parseMimeTypes,
  parseSize,
} from '../src/settings.ts'

const KB = 1024
const MB = 1024 * KB
const GB = 1024 * MB

test('sizes parse with or without a unit', () => {
  const cases: [string, number][] = [
    ['1 MB', MB],
    ['1.5mb', 1.5 * MB],
    ['500 KB', 500 * KB],
    ['2GB', 2 * GB],
    ['  10  ', 10 * MB],
    ['0', 0],
    ['512 b', 512],
  ]
  for (const [text, bytes] of cases) assert.deepEqual(parseSize(text), { ok: true, value: bytes })
})

test('sizes that make no sense are refused with a hint', () => {
  for (const text of ['', 'big', '-1 MB', '1 TB', '1,5 MB', '1e3', '1 MB 2', 'MB']) {
    const r = parseSize(text)
    assert.equal(r.ok, false, text)
    if (!r.ok) assert.match(r.error, /\d/, 'the hint shows an example')
  }
  assert.equal(parseSize('2000 GB').ok, false, 'over the cap')
  assert.deepEqual(parseSize('1024 GB'), { ok: true, value: MAX_MIN_BYTES })
})

test('sizes print in the largest whole-enough unit and round-trip', () => {
  assert.equal(formatSize(MB), '1 MB')
  assert.equal(formatSize(1.5 * MB), '1.5 MB')
  assert.equal(formatSize(700 * KB), '700 KB')
  assert.equal(formatSize(4 * GB), '4 GB')
  assert.equal(formatSize(0), '0 B')
  for (const bytes of [0, 512, MB, 1.5 * MB, 3 * GB])
    assert.deepEqual(parseSize(formatSize(bytes)), { ok: true, value: bytes })
})

test('domains accept hosts, pasted links and wildcards', () => {
  assert.deepEqual(
    parseDomains('example.org\nhttps://CDN.Example.net/path?q=1, *.mirror.io .lead.dot trail.dot.'),
    {
      values: ['example.org', 'cdn.example.net', 'mirror.io', 'lead.dot', 'trail.dot'],
      invalid: [],
    },
  )
  assert.deepEqual(parseDomains('a.com a.com A.COM').values, ['a.com'], 'duplicates dropped')
  assert.deepEqual(parseDomains('').values, [])
})

test('bad domains are reported as typed, not silently dropped', () => {
  assert.deepEqual(
    parseDomains('com localhost -bad.org bad-.org a..b exa_mple.org ok.org http://'),
    {
      values: ['ok.org'],
      invalid: ['com', 'localhost', '-bad.org', 'bad-.org', 'a..b', 'exa_mple.org', 'http://'],
    },
  )
  assert.deepEqual(parseDomains(`${'a'.repeat(64)}.org`).invalid.length, 1, 'label over 63')
})

test('file types accept dots and wildcards, refuse junk', () => {
  assert.deepEqual(parseExtensions('iso .ZIP *.tar.gz *.7z mkv'), {
    values: ['iso', 'zip', '7z', 'mkv'],
    invalid: ['*.tar.gz'],
  })
  assert.deepEqual(parseExtensions('. ** a/b x'.concat('y'.repeat(16))).values, [])
})

test('MIME types take exact types and families', () => {
  assert.deepEqual(parseMimeTypes('application/zip video/* Audio/MPEG text html/ */*'), {
    values: ['application/zip', 'video/*', 'audio/mpeg'],
    invalid: ['text', 'html/', '*/*'],
  })
})

test('stored rules are cleaned before use', () => {
  assert.deepEqual(normalizeRules(undefined), DEFAULT_RULES)
  assert.deepEqual(normalizeRules('junk'), DEFAULT_RULES)
  assert.deepEqual(normalizeRules({ enabled: false }), { ...DEFAULT_RULES, enabled: false })
  assert.deepEqual(
    normalizeRules({
      enabled: 'yes',
      minBytes: -5,
      extensions: ['.ISO', 7, 'iso', '../x'],
      includeDomains: 'example.org',
      excludeDomains: ['https://ads.example.com/x', 'com'],
      mimeTypes: ['video/*', null],
    }),
    {
      ...DEFAULT_RULES,
      extensions: ['iso'],
      excludeDomains: ['ads.example.com'],
      mimeTypes: ['video/*'],
    },
  )
  for (const minBytes of [NaN, Infinity, MAX_MIN_BYTES + 1, '5'])
    assert.equal(normalizeRules({ minBytes }).minBytes, DEFAULT_RULES.minBytes, String(minBytes))
  assert.equal(normalizeRules({ minBytes: 0 }).minBytes, 0, 'zero means take everything')
})
