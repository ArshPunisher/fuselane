import { expect, test } from '@playwright/test'

// A jobs event replaces the whole list; rows whose data did not change must keep their
// object, so memoised rows are not re-rendered (thousands of finished downloads).

test('a refreshed list keeps the objects of rows that did not change', async ({ page }) => {
  await page.goto('/?empty=1')
  const out = await page.evaluate(async () => {
    const path = '/src/lib/store.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/store')
    const a = { id: 1, name: 'a.iso', written: 10, networks: [{ n: 'en0' }] }
    const b = { id: 2, name: 'b.iso', written: 20, networks: [] as { n: string }[] }
    const prev = [a, b] as never[]
    // Same data in fresh objects (as every IPC event delivers it).
    const same = m.shareUnchanged(prev, JSON.parse(JSON.stringify(prev)))
    const moved = m.shareUnchanged(prev, JSON.parse(JSON.stringify([b, a])))
    const changed = m.shareUnchanged(prev, JSON.parse(JSON.stringify([{ ...a, written: 11 }, b])))
    const added = m.shareUnchanged(prev, JSON.parse(JSON.stringify([a, b, { id: 3 }])))
    return {
      sameArray: same === prev,
      sameA: same[0] === a,
      movedKept: moved[0] === b && moved[1] === a,
      changedA: changed[0] === a,
      changedB: changed[1] === b,
      addedKept: added[0] === a && added[1] === b && added.length === 3,
    }
  })
  expect(out).toEqual({
    sameArray: true,
    sameA: true,
    movedKept: true,
    changedA: false,
    changedB: true,
    addedKept: true,
  })
})

test('the list still updates its rows when a download advances', async ({ page }) => {
  await page.goto('/')
  const row = page.locator('li.row', { hasText: 'ubuntu-26.04' })
  const first = await row.locator('.row-meta .num').textContent()
  await expect
    .poll(async () => row.locator('.row-meta .num').textContent(), { timeout: 8000 })
    .not.toBe(first)
})
