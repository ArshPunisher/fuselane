import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEFAULT_RULES } from '@fuselane/capture'
import { readForm, toForm, type FormValues } from '../lib/settings-form.ts'
import { appStatus } from '../lib/status.ts'

const MB = 1024 * 1024
const form = (v: Partial<FormValues> = {}): FormValues => ({ ...toForm(DEFAULT_RULES), ...v })

test('the defaults show as typed text and read back unchanged', () => {
  assert.deepEqual(toForm(DEFAULT_RULES), {
    enabled: true,
    minSize: '1 MB',
    includeDomains: '',
    excludeDomains: '',
    extensions: '',
    mimeTypes: '',
  })
  assert.deepEqual(readForm(form()), { ok: true, rules: DEFAULT_RULES })
})

test('a filled form becomes clean rules', () => {
  const r = readForm({
    enabled: false,
    minSize: '20 mb',
    includeDomains: 'https://releases.ubuntu.com/24.04/\n*.archive.org',
    excludeDomains: 'ads.example.com, ads.example.com',
    extensions: '.iso ZIP',
    mimeTypes: 'video/*',
  })
  assert.deepEqual(r, {
    ok: true,
    rules: {
      enabled: false,
      minBytes: 20 * MB,
      includeDomains: ['releases.ubuntu.com', 'archive.org'],
      excludeDomains: ['ads.example.com'],
      extensions: ['iso', 'zip'],
      mimeTypes: ['video/*'],
    },
  })
  if (r.ok) assert.deepEqual(readForm(toForm(r.rules)), r, 'round-trips')
})

test('every bad field gets its own message naming what was typed', () => {
  const r = readForm(
    form({
      minSize: 'lots',
      includeDomains: 'localhost',
      excludeDomains: 'com example.org',
      extensions: 'a/b',
      mimeTypes: 'zip',
    }),
  )
  assert.equal(r.ok, false)
  if (r.ok) return
  assert.match(r.errors.minSize!, /1 MB/)
  assert.equal(
    r.errors.includeDomains,
    '“localhost” isn’t a site name. Use something like example.org.',
  )
  assert.equal(r.errors.excludeDomains, '“com” isn’t a site name. Use something like example.org.')
  assert.match(r.errors.extensions!, /“a\/b”.*iso or zip/)
  assert.match(r.errors.mimeTypes!, /“zip”.*application\/zip/)
})

test('several bad entries are all listed', () => {
  const r = readForm(form({ extensions: 'iso a/b c*d' }))
  assert.equal(r.ok, false)
  if (!r.ok) assert.match(r.errors.extensions!, /^“a\/b”, “c\*d” aren’t file types/)
})

test('a site in both lists is refused, since it can never be taken', () => {
  const r = readForm(form({ includeDomains: 'a.org b.org', excludeDomains: 'B.org' }))
  assert.deepEqual(r, {
    ok: false,
    errors: { excludeDomains: '“b.org” is in both lists. Remove it from one of them.' },
  })
})

test('the status line tells the three app states apart', async () => {
  assert.deepEqual(await appStatus(async () => ({ type: 'pong', app: { version: '0.1.0' } })), {
    state: 'connected',
    text: 'Connected to Fuselane 0.1.0.',
  })
  for (const reply of [{ type: 'pong' }, { error: 'app not running' }, undefined, 'junk'])
    assert.equal((await appStatus(async () => reply)).state, 'not-running', JSON.stringify(reply))
  assert.equal(
    (
      await appStatus(async () => {
        throw new Error('Specified native messaging host not found.')
      })
    ).state,
    'not-installed',
  )
})
