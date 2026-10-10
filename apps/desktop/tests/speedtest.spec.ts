import { expect, test, type Page } from '@playwright/test'

// The Speedtest tab, the Usage tab, and tabs that stay put (demo backend;
// ?fast=1 shortens the demo's speed test).

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('favicon')) errors.push(m.text())
  })
  page.on('pageerror', (e) => errors.push(e.message))
  ;(page as Page & { errors: string[] }).errors = errors
})
test.afterEach(async ({ page }) => {
  expect((page as Page & { errors: string[] }).errors).toEqual([])
})

const nav = (page: Page, name: string) =>
  page.getByRole('navigation', { name: 'Main' }).getByRole('button', { name })

test('Speedtest has its own tab, and Networks no longer has a Check tab', async ({ page }) => {
  await page.goto('/?drop=0')
  await nav(page, 'Networks').click()
  await expect(page.getByRole('radio', { name: 'Setup' })).toBeVisible()
  await expect(page.getByRole('radio', { name: 'Usage' })).toBeVisible()
  await expect(page.getByRole('radio', { name: 'Check' })).toHaveCount(0)
  await nav(page, 'Speedtest').click()
  await expect(page.getByRole('heading', { level: 1, name: 'Speedtest' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Start' })).toBeVisible()
  // Cmd/Ctrl+5 opens it too.
  await nav(page, 'Downloads').click()
  await page.keyboard.press('ControlOrMeta+5')
  await expect(page.getByRole('heading', { level: 1, name: 'Speedtest' })).toBeVisible()
})

test('a speed test runs live, step by step, then shows each network and all together', async ({
  page,
}) => {
  await page.goto('/?drop=0&fast=1')
  await nav(page, 'Speedtest').click()
  const page_ = page.getByRole('region', { name: 'Speedtest' })
  await expect(page_).toContainText('Wi-Fi: no internet 2 times this week')
  await page_.getByRole('button', { name: 'Start' }).click()
  await expect(page_.getByRole('status')).toContainText('Testing')
  const steps = page_.getByRole('list', { name: 'Steps' })
  await expect(steps.getByRole('listitem')).toHaveText(['Ping', 'Download', 'Upload', 'Together'])
  await expect(steps.locator('[aria-current="step"]')).toHaveText('Download', { timeout: 4000 })
  // The live figure moves.
  const value = page_.locator('.gauge-value')
  await expect(value).not.toHaveText('0.0')
  await expect(page_.getByRole('button', { name: 'Stop' })).toBeVisible()
  // Done: the dial rests on everything together, Start becomes Again.
  await expect(page_.getByRole('button', { name: 'Again' })).toBeVisible({ timeout: 15000 })
  await expect(page_.locator('.gauge-value')).toHaveText('262')
  await expect(page_.getByRole('status')).toContainText('Last test')
  const together = page_.locator('.speed-together')
  await expect(together).toContainText('262')
  await expect(together).toContainText('1.9× your fastest network')
  const ethernet = page_.getByRole('article').filter({ hasText: 'Ethernet' })
  await expect(ethernet).toContainText('138')
  await expect(ethernet).toContainText('96.0')
  await expect(ethernet).toContainText('Airtel Broadband')
  await expect(page_.getByRole('article').filter({ hasText: 'Wi-Fi' })).toContainText(
    'Slows down while busy',
  )
  await page_.getByRole('button', { name: 'Report for your provider' }).click()
})

test('stopping a speed test part way keeps what finished and can start again', async ({ page }) => {
  await page.goto('/?drop=0&fast=1')
  await nav(page, 'Speedtest').click()
  const page_ = page.getByRole('region', { name: 'Speedtest' })
  await page_.getByRole('button', { name: 'Start' }).click()
  // Pressing Start again while it runs can't start a second test.
  await expect(page_.getByRole('button', { name: 'Start' })).toHaveCount(0)
  await expect(page_.getByRole('article').filter({ hasText: 'Wi-Fi' })).toBeVisible({
    timeout: 6000,
  })
  await page_.getByRole('button', { name: 'Stop' }).click()
  await expect(page_.getByRole('button', { name: 'Again' })).toBeVisible({ timeout: 4000 })
  await expect(page_.locator('.speed-together')).toHaveCount(0)
  await expect(page_.getByRole('article').filter({ hasText: 'Wi-Fi' })).toBeVisible()
  // And it can run again.
  await page_.getByRole('button', { name: 'Again' }).click()
  await expect(page_.getByRole('button', { name: 'Stop' })).toBeVisible()
})

test('Usage shows this month, what moves right now, 30 days and each network', async ({ page }) => {
  await page.goto('/?drop=0')
  await nav(page, 'Networks').click()
  await page.getByRole('radio', { name: 'Usage' }).click()
  const used = page.getByRole('region', { name: 'Data used' })
  await expect(used).toContainText('This month')
  await expect(used.locator('.use-big')).toContainText(/GB/)
  await expect(used).toContainText('Right now')
  await expect(used.locator('.use-live-list li')).toHaveCount(3)
  await expect(
    used.getByRole('img', { name: /Data used per day for the last 30 days/ }),
  ).toBeVisible()
  await expect(used.getByRole('article').filter({ hasText: 'iPhone USB' })).toContainText(/GB|MB/)
  await expect(used.locator('.use-axis')).toContainText('Today')
})

test('Usage with nothing counted says so instead of showing a blank page', async ({ page }) => {
  await page.goto('/?empty=1&usage=none')
  await nav(page, 'Networks').click()
  await page.getByRole('radio', { name: 'Usage' }).click()
  await expect(page.getByRole('heading', { name: 'Nothing counted yet' })).toBeVisible()
})

test("Usage still shows the month's totals when there's no day-by-day history yet", async ({
  page,
}) => {
  await page.goto('/?empty=1&usage=month')
  await nav(page, 'Networks').click()
  await page.getByRole('radio', { name: 'Usage' }).click()
  const used = page.getByRole('region', { name: 'Data used' })
  await expect(used.locator('.use-big')).toContainText(/GB|MB/)
  await expect(used.locator('.use-axis')).toContainText('Today')
})

test('the tabs in Send and Networks stay in the same place whichever tab is open', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  const x = async () => (await page.locator('.page-head .segmented').boundingBox())!.x
  await nav(page, 'Send').click()
  await page.getByRole('radio', { name: 'Nearby' }).click()
  const nearby = await x()
  await page.getByRole('radio', { name: 'Link' }).click()
  expect(Math.abs((await x()) - nearby)).toBeLessThan(1)
  await nav(page, 'Networks').click()
  await page.getByRole('radio', { name: 'Setup' }).click()
  const setup = await x()
  await page.getByRole('radio', { name: 'Usage' }).click()
  expect(Math.abs((await x()) - setup)).toBeLessThan(1)
})

test('on a phone the five tabs fit and nothing scrolls sideways', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/?drop=0&fast=1')
  await nav(page, 'Speedtest').click()
  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.locator('.gauge-value')).toBeVisible({ timeout: 4000 })
  for (const view of ['Speedtest', 'Downloads', 'Send', 'Networks', 'Settings']) {
    await nav(page, view).click()
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth - innerWidth),
      view,
    ).toBeLessThanOrEqual(0)
  }
})
