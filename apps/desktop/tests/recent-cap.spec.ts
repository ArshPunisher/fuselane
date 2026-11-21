import { expect, test } from '@playwright/test'

// Thousands of finished downloads must not all be in the page at once.

test('Recent shows the latest 200 and a button for older ones', async ({ page }) => {
  await page.goto('/?many=500')
  const recent = page
    .getByRole('region', { name: /Recent/ })
    .or(page.locator('section', { has: page.getByRole('heading', { name: /Recent/ }) }))
  const rows = recent.locator('li.row')
  await expect(rows).toHaveCount(200)
  const more = page.getByRole('button', { name: /Show older/ })
  await expect(more).toBeVisible()
  await more.click()
  await expect(rows).toHaveCount(400)
  await page.getByRole('button', { name: /Show older/ }).click()
  await expect(page.getByRole('button', { name: /Show older/ })).toHaveCount(0)
  expect(await rows.count()).toBeGreaterThan(400)
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
      .locator('li.row'),
  ).toHaveCount(200)
})
