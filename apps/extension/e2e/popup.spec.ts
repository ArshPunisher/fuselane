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

test('the popup opens, explains the app is missing, and remembers its switch', async () => {
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  await page.goto(`chrome-extension://${id}/popup.html`)
  await expect(page.getByRole('heading', { name: 'Fuselane' })).toBeVisible()
  // No native host is registered in this profile, so the status says what to do.
  await expect(page.getByRole('status').first()).not.toHaveText('Checking for the Fuselane app…', {
    timeout: 10_000,
  })
  const toggle = page.getByLabel('Hand big downloads to Fuselane')
  const was = await toggle.isChecked()
  await toggle.setChecked(!was)
  await page.reload()
  await expect(page.getByLabel('Hand big downloads to Fuselane')).toBeChecked({ checked: !was })
  await expect(page.getByRole('link', { name: /Size, sites and file types/ })).toBeVisible()
  expect(errors).toEqual([])
})
