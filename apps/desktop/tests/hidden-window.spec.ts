import { expect, test, type Page } from '@playwright/test'

// Fuselane lives in the tray: with its window hidden, the page must stop asking the
// backend for things nobody can see, and catch up the moment it is shown.

const hide = (page: Page, hidden: boolean) =>
  page.evaluate((h) => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => h })
    document.dispatchEvent(new Event('visibilitychange'))
  }, hidden)

test('a hidden window stops polling the backend and catches up when shown', async ({ page }) => {
  await page.goto('/?empty=1')
  await expect(page.getByText('Demo data')).toBeVisible()
  await page.evaluate(async () => {
    const path = '/src/lib/store.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/store')
    const b = m.useApp.getState().backend as unknown as Record<string, (...a: unknown[]) => unknown>
    const w = window as unknown as { __calls: number }
    w.__calls = 0
    for (const name of ['allowances', 'usageHistory', 'watchState', 'listNetworks']) {
      const real = b[name]!.bind(b)
      b[name] = (...a: unknown[]) => {
        w.__calls++
        return real(...a)
      }
    }
  })
  const calls = () => page.evaluate(() => (window as unknown as { __calls: number }).__calls)
  await page.getByRole('button', { name: 'Networks' }).first().click()
  await page.getByRole('radio', { name: 'Usage' }).click()
  await expect.poll(calls).toBeGreaterThan(0)

  await hide(page, true)
  await page.waitForTimeout(300)
  const before = await calls()
  await page.waitForTimeout(11_000) // longer than every poll interval
  expect(await calls()).toBe(before)

  await hide(page, false)
  await expect.poll(calls, { timeout: 2000 }).toBeGreaterThan(before)
})

test('a rate history that has nothing to add is not replaced (no re-render)', async ({ page }) => {
  await page.goto('/?empty=1')
  const out = await page.evaluate(async () => {
    const path = '/src/lib/rates.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/rates')
    const quiet = { names: ['en0'], rates: [[0], [0]] }
    const same = m.appendRates(quiet, ['en0'], [0])
    const grew = m.appendRates(quiet, ['en0'], [500])
    const renamed = m.appendRates(quiet, ['en7'], [0])
    const capped = m.appendRates(
      { names: ['en0'], rates: Array.from({ length: 300 }, () => [1]) },
      ['en0'],
      [2],
    )
    return {
      same: same === quiet,
      grew: grew.rates.length,
      renamed: renamed.names[0] + ':' + renamed.rates.length,
      capped: capped.rates.length,
      last: capped.rates[299],
    }
  })
  expect(out).toEqual({ same: true, grew: 3, renamed: 'en7:1', capped: 300, last: [2] })
})
