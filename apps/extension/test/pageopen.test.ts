import { test } from 'node:test'
import assert from 'node:assert/strict'
import { canOpenPage, openPageOutcome } from '../lib/pageopen.ts'

test('any web page can be opened in the app, whatever site it is', () => {
  // L-127: the button is offered on every web page, never chosen by site.
  for (const u of [
    'https://www.youtube.com/watch?v=aqz-KE-bpKQ',
    'https://m.youtube.com/shorts/abc',
    'https://example.org/downloads/',
    'http://intranet.local/page',
  ]) {
    assert.equal(canOpenPage(u), true, u)
  }
  for (const u of [
    undefined,
    '',
    'edge://extensions/',
    'chrome://newtab/',
    'about:blank',
    'file:///C:/x.html',
    'not a url',
    `https://example.org/${'a'.repeat(8200)}`,
  ]) {
    assert.equal(canOpenPage(u), false, String(u).slice(0, 40))
  }
})

test('the popup closes once the app has opened the page', () => {
  assert.deepEqual(openPageOutcome({ v: 1, type: 'page.open.opened' }, 'connected'), {
    close: true,
  })
  // The app was closed and the host started it: the answer still counts.
  assert.deepEqual(openPageOutcome({ v: 1, type: 'page.open.opened' }, 'not-running'), {
    close: true,
  })
})

test('a clear message when the app is missing, too old or not answering', () => {
  const declined = (reason: string) => ({
    v: 1,
    type: 'download.declined',
    reason,
    fallback: 'browser',
  })
  const say = (reply: unknown, state: Parameters<typeof openPageOutcome>[1]) => {
    const o = openPageOutcome(reply, state)
    return o.close ? 'closed' : o.text
  }
  assert.match(say(null, 'not-installed'), /needs the Fuselane app/)
  assert.match(say(declined('invalid'), 'connected'), /Update Fuselane/)
  assert.match(say(declined('unsupported'), 'not-running'), /didn't answer/)
  assert.match(say(undefined, 'connected'), /didn't answer/)
  assert.match(say('nonsense', 'connected'), /didn't answer/)
})
