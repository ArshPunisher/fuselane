import { expect, test, type Page } from '@playwright/test'

// Live state is pruned, and the idle canvas does not redraw.

/** The demo page with a hook that counts requestAnimationFrame callbacks. */
async function countFrames(page: Page, ms: number): Promise<number> {
  return page.evaluate(
    (span) =>
      new Promise<number>((resolve) => {
        const w = window as unknown as { __frames?: number; __patched?: boolean }
        const real = window.requestAnimationFrame.bind(window)
        if (!w.__patched) {
          w.__patched = true
          w.__frames = 0
          window.requestAnimationFrame = (cb) =>
            real((t) => {
              w.__frames = (w.__frames ?? 0) + 1
              cb(t)
            })
        }
        const start = w.__frames ?? 0
        setTimeout(() => resolve((w.__frames ?? 0) - start), span)
      }),
    ms,
  )
}

test('live numbers and history of a removed download are forgotten', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  const out = await page.evaluate(async () => {
    const path = '/src/lib/store.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/store')
    const before = Object.keys(m.useApp.getState().live)
    // The list now holds only job 999: everything else was removed.
    m.useApp.setState((s) => ({
      live: { ...s.live, 999: s.live[Number(before[0])]! },
      history: { ...s.history, 999: s.history[Number(before[0])]! },
    }))
    const kept = m.forgetGone(m.useApp.getState(), [{ id: 999 }] as never)
    return { before, liveAfter: Object.keys(kept.live), historyAfter: Object.keys(kept.history) }
  })
  expect(out.before.length).toBeGreaterThan(0)
  expect(out.liveAfter).toEqual(['999'])
  expect(out.historyAfter).toEqual(['999'])
})

test('a finished job in the detail pane does not redraw the canvas every frame', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  await page.getByText('dataset-shard-0042.tar.zst').click()
  await expect(page.locator('article.detail .speed-sub')).toHaveText('Paused')
  await page.waitForTimeout(1500) // easing settles
  const frames = await countFrames(page, 1000)
  expect(frames).toBeLessThan(10) // about 60 a second while it loops for nothing
})

test('a running job still animates', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  // Moving at all is the point (an idle ring draws none); a slow CI browser
  // manages about 15 frames a second, so don't ask for a smooth 60.
  expect(await countFrames(page, 1000)).toBeGreaterThan(5)
})
