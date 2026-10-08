import { test } from 'node:test'
import assert from 'node:assert/strict'
import { cookieHeader } from '../src/cookies.ts'

test('cookies join in order, first of each name wins', () => {
  assert.equal(
    cookieHeader([
      { name: 'session', value: 'specific' },
      { name: 'theme', value: 'dark' },
      { name: 'session', value: 'general' },
    ]),
    'session=specific; theme=dark',
  )
  assert.equal(cookieHeader([]), '')
})

test('cookies that could split or inject a header are dropped', () => {
  assert.equal(
    cookieHeader([
      { name: 'ok', value: '1' },
      { name: 'bad;name', value: 'x' },
      { name: 'a b', value: 'x' },
      { name: 'evil', value: 'x\r\nX-Injected: 1' },
      { name: 'semi', value: 'a;b' },
      { name: '', value: 'x' },
    ]),
    'ok=1',
  )
})
