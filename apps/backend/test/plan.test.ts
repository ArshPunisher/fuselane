import { describe, expect, it } from 'vitest'
import { MAX_PARTS, MAX_SIZE, MIB, plan } from '../src/plan'

describe('part plan', () => {
  it('uses 16 MiB parts for ordinary files, rounded to whole MiB', () => {
    expect(plan(1)).toEqual({ partSize: 16 * MIB, partCount: 1 })
    expect(plan(16 * MIB)).toEqual({ partSize: 16 * MIB, partCount: 1 })
    expect(plan(16 * MIB + 1)).toEqual({ partSize: 16 * MIB, partCount: 2 })
    expect(plan(10 * 1024 * MIB).partCount).toBe(640)
  })
  it('grows parts for huge files so they stay under 10,000 parts', () => {
    for (const size of [MAX_SIZE, 141 * 1024 * MIB, 5 * 1024 * 1024 * MIB]) {
      const p = plan(size)
      expect(p.partCount).toBeLessThanOrEqual(MAX_PARTS)
      expect(p.partSize % MIB).toBe(0)
      expect(p.partSize * p.partCount).toBeGreaterThanOrEqual(size)
      expect(p.partSize * (p.partCount - 1)).toBeLessThan(size)
    }
  })
})
