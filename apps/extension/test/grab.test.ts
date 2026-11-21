import { test } from 'node:test'
import assert from 'node:assert/strict'
import { clickCanGrab, createGrabCap, type ClickLike } from '../lib/grab.ts'

// F1: a page must not be able to push downloads into Fuselane by scripting clicks.

const click = (over: Partial<ClickLike> = {}): ClickLike => ({
  isTrusted: true,
  defaultPrevented: false,
  button: 0,
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...over,
})

test("a person's plain left click on a file link can be grabbed", () => {
  assert.equal(clickCanGrab(click()), true)
})

test('a click a page made with a.click() or dispatchEvent is never grabbed', () => {
  assert.equal(clickCanGrab(click({ isTrusted: false })), false)
})

test('modified, other-button and already-handled clicks stay with the browser', () => {
  for (const over of [
    { altKey: true },
    { ctrlKey: true },
    { metaKey: true },
    { shiftKey: true },
    { button: 1 },
    { button: 2 },
    { defaultPrevented: true },
  ]) {
    assert.equal(clickCanGrab(click(over)), false, JSON.stringify(over))
  }
})

test('more than three grabs in two seconds from one tab go to the browser', () => {
  const cap = createGrabCap(3, 2000)
  assert.deepEqual(
    [0, 100, 200].map((t) => cap.allow(7, t)),
    [true, true, true],
  )
  assert.equal(cap.allow(7, 300), false, 'the fourth within the window')
  assert.equal(cap.allow(8, 300), true, 'another tab is counted on its own')
  assert.equal(cap.allow(7, 2100), true, 'the window slid past the first grabs')
})

test('refused grabs do not extend the wait, and a quiet tab is forgotten', () => {
  const cap = createGrabCap(2, 1000)
  cap.allow(1, 0)
  cap.allow(1, 10)
  for (let t = 20; t < 900; t += 20) assert.equal(cap.allow(1, t), false)
  assert.equal(cap.allow(1, 1011), true, 'only the allowed grabs count')
  assert.equal(cap.size(), 1)
  assert.equal(cap.allow(2, 5000), true)
  assert.equal(cap.size(), 1, 'tab 1 had gone quiet and was dropped')
})
