import { expect, test } from '@playwright/test'

// Thousands of finished downloads must not all be in the page at once.

test('Recent shows the latest 200 and a button for older ones', async ({ page }) => {
  await page.goto('/?many=500')
  const recent = page
    .getByRole('region', { name: /Recent/ })
    .or(page.locator('section', { has: page.getByRole('heading', { name: /Recent/ }) }))
  const rows = recent.locator('li.row[data-status="completed"]')
  await expect(rows).toHaveCount(200, { timeout: 15_000 })
  const more = page.getByRole('button', { name: /Show older/ })
  await expect(more).toBeVisible()
  await more.click()
  await expect(rows).toHaveCount(400, { timeout: 15_000 })
  await page.getByRole('button', { name: /Show older/ }).click()
  await expect(page.getByRole('button', { name: /Show older/ })).toHaveCount(0)
  expect(await rows.count()).toBeGreaterThan(400)
})

// Only finished downloads are held back: a paused or failed one keeps its Resume or
// Retry in view, however long the history is.
test('unfinished downloads are never hidden behind Show older', async ({ page }) => {
  await page.goto('/?many=500')
  const recent = page.locator('section', { has: page.getByRole('heading', { name: /Recent/ }) })
  await expect(recent.locator('li.row[data-status="completed"]')).toHaveCount(200)
  // Every ninth of the 500 failed (56) plus the sample failed one, all on screen at once.
  await expect(recent.locator('li.row[data-status="failed"]')).toHaveCount(57)
  await expect(page.getByRole('button', { name: /Show older/ })).toContainText('more')
})

test('searching starts again from the latest, and finds an older download', async ({ page }) => {
  await page.goto('/?many=500')
  await page.getByRole('button', { name: /Show older/ }).click()
  const search = page.getByRole('searchbox', { name: 'Search downloads' })
  await search.fill('499-')
  await expect(page.locator('.row-name', { hasText: '499-' }).first()).toBeVisible()
  await search.fill('')
  await expect(
    page
      .locator('section', { has: page.getByRole('heading', { name: /Recent/ }) })
      .locator('li.row[data-status="completed"]'),
  ).toHaveCount(200)
})
