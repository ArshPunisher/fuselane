import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ACTIVE, orphanedParts, type SelfJob } from '../lib/selfjobs.ts'

// F3: private download files (up to 2 GB each) must not outlive their job.

test('only files of jobs that are gone are swept, and only ours', () => {
  const names = ['xa1.part', 'xb2.part', 'xc3.part', 'notes.txt', 'xd4.part.crswap', '.part']
  const keep = new Set(['xb2'])
  assert.deepEqual(orphanedParts(names, keep), ['xa1.part', 'xc3.part'])
  assert.deepEqual(orphanedParts([], keep), [])
})

test('a job that can still run or be saved keeps its file', () => {
  const status = (s: SelfJob['status']) => ACTIVE.includes(s)
  for (const s of ['queued', 'running', 'paused', 'saving'] as const) assert.ok(status(s), s)
  for (const s of ['completed', 'failed', 'too-big'] as const) assert.ok(!status(s), s)
})
