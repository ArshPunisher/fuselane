import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'

// Accessibility checks (P3): every main screen and dialog, in light and dark,
// must have no serious or critical axe-core violations (WCAG 2.2 A and AA).
async function audit(page: Page, what: string) {
  // Measure once rows and dialogs have finished fading in (live orbs and
  // spinners elsewhere keep moving and are left out).
  await page.waitForFunction(() =>
    document
      .getAnimations()
      .filter((a) => {
        const t = (a.effect as KeyframeEffect | null)?.target
        return t instanceof Element && t.closest('dialog, .row, .send-item, .peer-row, .notice')
      })
      .filter((a) => a.effect?.getTiming().iterations !== Infinity)
      // Progress bars keep easing their width; only fades and rises matter here.
      .filter(
        (a) =>
          !('transitionProperty' in a) ||
          ['opacity', 'transform'].includes(String(a.transitionProperty)),
      )
      .every((a) => a.playState !== 'running'),
  )
  const r = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze()
  const bad = r.violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)
  expect(bad, `${what}: ${bad.join('\n')}`).toEqual([])
}

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`${scheme} mode`, () => {
    test.use({ colorScheme: scheme })

    test('downloads and a download in progress', async ({ page }) => {
      await page.goto('/?freeze=3')
      await expect(page.getByTestId('fuse-core')).toBeVisible()
      await audit(page, 'downloads')
    })

    test('a torrent', async ({ page }) => {
      await page.goto('/?freeze=3')
      await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
      await expect(page.getByRole('table', { name: 'Networks in this torrent' })).toBeVisible()
      await audit(page, 'torrent detail')
    })

    test('new download and choose files', async ({ page }) => {
      await page.goto('/?torrents=0')
      await page.getByRole('button', { name: 'New download' }).first().click()
      await expect(page.getByRole('dialog', { name: 'New download' })).toBeVisible()
      await audit(page, 'new download')
      await page.getByRole('button', { name: 'Open .torrent…' }).click()
      await expect(page.getByRole('dialog', { name: 'Choose files' })).toBeVisible()
      await audit(page, 'choose files')
    })

    test('networks and settings', async ({ page }) => {
      await page.goto('/?share=1')
      await page.getByRole('button', { name: 'Networks' }).first().click()
      await expect(page.getByRole('heading', { level: 1, name: 'Networks' })).toBeVisible()
      await audit(page, 'networks')
      await page.getByRole('button', { name: 'Settings' }).first().click()
      await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible()
      await page.getByRole('switch', { name: 'Share torrents after downloading' }).click()
      await audit(page, 'settings')
    })

    test('a network behind a sign-in page', async ({ page }) => {
      await page.goto('/?portal=en0')
      await page.getByRole('button', { name: 'Networks' }).first().click()
      await expect(page.getByRole('note')).toBeVisible()
      await audit(page, 'sign-in needed')
    })

    test('empty list', async ({ page }) => {
      await page.goto('/?empty=1')
      await expect(page.getByText('Nothing downloading yet')).toBeVisible()
      await audit(page, 'empty')
    })
  })
}
