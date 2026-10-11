import { test, expect, chromium, type BrowserContext } from '@playwright/test'
import { existsSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// The popup in a real Chromium (STEPS 7.8): it opens, says whether the app is
// there, and its switch is remembered. (Listing a page's videos and feeds needs
// the toolbar click that grants activeTab, which a test can't make; that logic
// is covered by the unit tests in test/.)
const extension = resolve(import.meta.dirname, '../.output/chrome-mv3')

let tmp: string
let context: BrowserContext
let id: string

test.beforeAll(async () => {
  expect(existsSync(join(extension, 'manifest.json')), 'run `wxt build` first').toBe(true)
  tmp = mkdtempSync(join(tmpdir(), 'fuselane-popup-'))
  context = await chromium.launchPersistentContext(join(tmp, 'profile'), {
    channel: 'chromium',
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  })
  const worker = context.serviceWorkers()[0] ?? (await context.waitForEvent('serviceworker'))
  id = new URL(worker.url()).host
})

test.afterAll(async () => {
  await context?.close()
  if (tmp) rmSync(tmp, { recursive: true, force: true })
})

test('the popup opens, offers the app, and its Fuselane switch turns everything off and on', async () => {
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  await page.goto(`chrome-extension://${id}/popup.html`)
  await expect(page.getByRole('heading', { name: 'Fuselane' })).toBeVisible()
  // No native host is registered in this profile: no app.
  await expect(page.locator('#chip')).toHaveText('No app', { timeout: 10_000 })
  const get = page.getByRole('link', { name: 'Get the app' })
  await expect(get).toBeVisible()
  expect(await get.getAttribute('href')).toMatch(/^https:\/\/fuselane\.app\/download\//)
  // Site access isn't given yet, so it's offered.
  await expect(page.getByRole('button', { name: 'Allow Fuselane on all sites' })).toBeVisible()
  // The tabs.
  await expect(page.getByRole('tab', { name: 'Downloads' })).toHaveAttribute(
    'aria-selected',
    'true',
  )
  await expect(page.getByText('No downloads yet')).toBeVisible()
  await page.getByRole('tab', { name: 'Speed' }).click()
  await expect(page.getByRole('button', { name: 'Start the test' })).toBeVisible()
  await page.getByRole('tab', { name: 'Downloads' }).click()
  // Off: only the big switch is left. It's remembered, and turns back on.
  await page.getByRole('button', { name: 'Turn Fuselane downloads off' }).click()
  const on = page.getByRole('switch', { name: 'Turn on' })
  await expect(on).toBeVisible()
  await expect(page.getByRole('tab', { name: 'Downloads' })).toBeHidden()
  await expect(page.locator('#chip')).toBeHidden()
  await page.reload()
  await expect(page.getByRole('switch', { name: 'Turn on' })).toBeVisible()
  await page.getByRole('switch', { name: 'Turn on' }).click()
  await expect(page.getByRole('tab', { name: 'Downloads' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('tab', { name: 'Downloads' })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Settings' }).first()).toBeVisible()
  await expect(page.getByRole('link', { name: 'by ClearCraft Labs' })).toHaveAttribute(
    'href',
    /^https:\/\/clearcraftlabs\.com\//,
  )
  expect(errors).toEqual([])
})
