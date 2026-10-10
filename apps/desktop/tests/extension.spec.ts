import { expect, test, type Page } from '@playwright/test'

// The Browser extension page (BROWSER-EXTENSION.md §7): which browsers have the
// extension, adding it from the app, and where browser downloads go.

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

const opens = (page: Page) =>
  page.evaluate(() => (window as unknown as { __demoBrowserOpens: string[] }).__demoBrowserOpens)

async function openPage(page: Page, query = '') {
  await page.goto(`/${query}`)
  await page.getByRole('button', { name: 'Extension' }).first().click()
  await expect(page.getByRole('heading', { level: 1, name: 'Browser extension' })).toBeVisible()
  return page.getByRole('list', { name: 'Your browsers' })
}

test('each browser shows its state and the one thing to do next', async ({ page }) => {
  const list = await openPage(page)
  const chrome = list.locator('[data-browser="chrome"]')
  await expect(chrome).toContainText('Connected')
  await expect(chrome).toContainText('version 0.3.0')
  await expect(chrome.getByRole('button')).toHaveCount(0)
  const edge = list.locator('[data-browser="edge"]')
  await expect(edge).toContainText('Not added')
  const firefox = list.locator('[data-browser="firefox"]')
  await expect(firefox).toContainText('Turned off')

  // Turning it back on opens Firefox's own extensions page, not the store.
  await firefox.getByRole('button', { name: 'Turn on' }).click()
  expect(await opens(page)).toEqual(['firefox:extensions'])

  // Adding it opens Edge at its store page; the page notices once it's there.
  await edge.getByRole('button', { name: 'Add to Microsoft Edge' }).click()
  expect(await opens(page)).toEqual(['firefox:extensions', 'edge:store'])
  await expect(edge).toContainText('Connected', { timeout: 8000 })
  await expect(edge.getByRole('button')).toHaveCount(0)
})

test('where browser downloads go: all, small ones in the browser, or none', async ({ page }) => {
  await openPage(page)
  const group = page.getByRole('radiogroup', { name: 'Where browser downloads go' })
  const small = group.getByRole('radio', { name: /Small files stay in the browser/ })
  await expect(small).toBeChecked()
  const size = page.getByRole('combobox', { name: 'Stay in the browser up to' })
  await expect(size).toHaveValue(String(1024 * 1024))
  await size.selectOption({ label: '2.0 GB' })

  await group.getByRole('radio', { name: /Everything to Fuselane/ }).check()
  await expect(size).toBeHidden()
  // Leaving the page and coming back reads the saved setting.
  await page.getByRole('button', { name: 'Settings' }).first().click()
  await page.getByRole('button', { name: 'Extension' }).first().click()
  await expect(group.getByRole('radio', { name: /Everything to Fuselane/ })).toBeChecked()

  await group.getByRole('radio', { name: /Keep downloads in the browser/ }).check()
  await page.getByRole('button', { name: 'Settings' }).first().click()
  await page.getByRole('button', { name: 'Extension' }).first().click()
  await expect(group.getByRole('radio', { name: /Keep downloads in the browser/ })).toBeChecked()
})

test('no browser found: says which ones work, and Downloads asks nothing', async ({ page }) => {
  await page.goto('/?browsers=none')
  await expect(page.getByRole('button', { name: 'Feeds', exact: true })).toBeVisible()
  await expect(page.getByTestId('extension-nudge')).toHaveCount(0)
  await page.getByRole('button', { name: 'Extension' }).first().click()
  await expect(page.getByText('No supported browser found.')).toBeVisible()
})

test('Downloads offers the extension until it is added or dismissed', async ({ page }) => {
  await page.goto('/?browsers=missing')
  const nudge = page.getByTestId('extension-nudge')
  await expect(nudge).toContainText('downloads from your browser come here')
  await nudge.getByRole('button', { name: 'Add to Chrome' }).click()
  expect(await opens(page)).toEqual(['chrome:store'])
  await expect(page.getByRole('heading', { level: 1, name: 'Browser extension' })).toBeVisible()

  await page.goto('/?browsers=missing')
  await page.getByTestId('extension-nudge').getByRole('button', { name: 'Not now' }).click()
  await expect(page.getByTestId('extension-nudge')).toHaveCount(0)
  // Dismissed stays dismissed.
  await page.reload()
  await expect(page.getByRole('button', { name: 'Feeds', exact: true })).toBeVisible()
  await expect(page.getByTestId('extension-nudge')).toHaveCount(0)
})

test('with the extension on somewhere, Downloads stays quiet', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Feeds', exact: true })).toBeVisible()
  await expect(page.getByTestId('extension-nudge')).toHaveCount(0)
})

test('at 375 px every tab label fits and the page never scrolls sideways', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 760 })
  await page.goto('/')
  await page.getByRole('button', { name: 'Extension' }).last().click()
  await expect(page.getByRole('heading', { level: 1, name: 'Browser extension' })).toBeVisible()
  const cut = await page
    .locator('.nav-tabs .nav-label')
    .evaluateAll((els) =>
      els.filter((e) => e.scrollWidth > e.clientWidth).map((e) => e.textContent),
    )
  expect(cut).toEqual([])
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})
