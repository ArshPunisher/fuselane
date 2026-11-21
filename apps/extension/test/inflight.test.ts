import { test } from 'node:test'
import assert from 'node:assert/strict'
import { skipWhileRunning } from '../lib/inflight.ts'

// F4: a slow app must not make the popup's polls pile up behind each other.

test('a call made while one is still running is skipped, not queued', async () => {
  let started = 0
  let release: () => void = () => {}
  const slow = skipWhileRunning(async () => {
    started++
    await new Promise<void>((r) => (release = r))
    return started
  })
  const first = slow()
  assert.equal(await slow(), undefined, 'skipped')
  assert.equal(await slow(), undefined, 'skipped again')
  assert.equal(started, 1)
  release()
  assert.equal(await first, 1)
  const next = slow()
  release()
  assert.equal(await next, 2, 'free again once the last one finished')
})

test('a failed call frees the next one', async () => {
  let n = 0
  const flaky = skipWhileRunning(async () => {
    if (++n === 1) throw new Error('host dropped')
    return n
  })
  await assert.rejects(flaky(), /host dropped/)
  assert.equal(await flaky(), 2)
})
