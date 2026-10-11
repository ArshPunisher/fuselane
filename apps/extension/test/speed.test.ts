import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DIAL_STOPS, dialFraction } from '../lib/speed.ts'

test('the dial numbers are evenly spaced and the needle moves evenly between them', () => {
  const last = DIAL_STOPS.length - 1
  DIAL_STOPS.forEach((m, i) => assert.equal(dialFraction(m), i / last))
  // Halfway between 100 and 250 is halfway between their marks.
  const i = DIAL_STOPS.indexOf(100)
  assert.equal(dialFraction(175), (i + 0.5) / last)
  assert.equal(dialFraction(-3), 0)
  assert.equal(dialFraction(Number.NaN), 0)
  assert.equal(dialFraction(5000), 1)
})
