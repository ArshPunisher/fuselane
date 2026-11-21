import { expect, test } from '@playwright/test'

// Shortcuts must not take over scrolling.

test('arrow keys and Space do not switch or pause a download while its detail is open', async ({
  browser,
}) => {
  const ctx = await browser.newContext({ viewport: { width: 800, height: 900 } })
  const page = await ctx.newPage()
  await page.goto('/')
  await page.getByText('ubuntu-26.04-desktop-amd64.iso').first().click()
  const detail = page.locator('article.detail')
  await expect(detail).toBeVisible()
  const title = await detail.getByRole('heading', { level: 1 }).textContent()
  await page.locator('main').focus()
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('Space')
  await page.waitForTimeout(300)
  await expect(detail.getByRole('heading', { level: 1 })).toHaveText(title ?? '')
  await expect(detail.getByRole('button', { name: 'Pause' })).toBeVisible()
  await ctx.close()
})
