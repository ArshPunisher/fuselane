import { expect, test } from '@playwright/test'

// L-35: data usage rounds down.

test('data usage rounds down: 4.96 GB of 5 never reads as 5.0 GB (L-35)', async ({ page }) => {
  await page.goto('/?empty=1')
  const out = await page.evaluate(async () => {
    const path = '/src/lib/format.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/format')
    return {
      used: m.usedBytes(4.96 * 1024 ** 3),
      exact: m.usedBytes(5 * 1024 ** 3),
      edge: m.usedBytes(1023.97 * 1024),
      small: m.usedBytes(512),
      shown: m.bytes(4.96 * 1024 ** 3), // plain sizes keep rounding to nearest
    }
  })
  const nb = ' '
  expect(out.used).toBe(`4.9${nb}GB`)
  expect(out.exact).toBe(`5.0${nb}GB`)
  expect(out.edge).toBe(`1,023${nb}KB`)
  expect(out.small).toBe(`512${nb}B`)
  expect(out.shown).toBe(`5.0${nb}GB`)
})
