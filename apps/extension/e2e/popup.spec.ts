import { test, expect, chromium, type BrowserContext } from '@playwright/test'
import { existsSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// The popup in a real Chromium (STEPS 7.8): it opens, says whether the app is
// there, and its switch is remembered. (Listing a page's file links and feeds needs
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

// L-127: "Open this page in Fuselane" is on every web page, YouTube included,
// and sends only the link. The toolbar click can't be made in a test, so the
// tab, the page script and the native host are stood in for.
test('This page offers Open this page in Fuselane on any web page and hands over only the link', async () => {
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  await page.addInitScript(() => {
    const w = window as unknown as { sent: unknown[]; shut: boolean; reply: unknown }
    w.sent = []
    w.shut = false
    // A reload can ask for another answer (sessionStorage outlives it).
    w.reply = JSON.parse(sessionStorage.getItem('reply') ?? '{"v":1,"type":"page.open.opened"}')
    // Only the three calls the popup makes here are stood in for.
    type Stub = {
      tabs: { query: unknown }
      scripting: { executeScript: unknown }
      runtime: { sendNativeMessage: unknown }
    }
    const c = (globalThis as unknown as { chrome: Stub }).chrome
    c.tabs.query = async () => [
      {
        id: 7,
        url: 'https://www.youtube.com/watch?v=aqz-KE-bpKQ',
        title: 'Cheema Y - Intro (Official Audio)',
      },
    ]
    c.scripting.executeScript = async () => [{ result: [] }]
    // Answers both ways the bindings may ask: a promise, or a callback.
    c.runtime.sendNativeMessage = (_host: string, m: unknown, done?: (r: unknown) => void) => {
      w.sent.push(m)
      // The app is running (an older one in the second half).
      const reply =
        (m as { type?: string }).type === 'ping'
          ? { v: 1, type: 'pong', app: { app: 'fuselane', version: '0.1.0-beta.14', v: 1 } }
          : w.reply
      if (done) setTimeout(() => done(reply))
      return Promise.resolve(reply)
    }
    window.close = () => {
      w.shut = true // not window.closed: that one is read-only
    }
  })
  await page.goto(`chrome-extension://${id}/popup.html`)
  await page.getByRole('tab', { name: 'This page' }).click()
  await expect(page.locator('#page-title')).toHaveText('Cheema Y - Intro (Official Audio)')
  await expect(page.getByText('No file links here.')).toBeVisible()
  const open = page.getByRole('button', { name: 'Open this page in Fuselane' })
  await expect(open).toBeVisible()
  await open.click()
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { shut: boolean }).shut))
    .toBe(true)
  const sent = await page.evaluate(() => (window as unknown as { sent: unknown[] }).sent)
  expect(sent).toContainEqual({
    v: 1,
    type: 'page.open',
    url: 'https://www.youtube.com/watch?v=aqz-KE-bpKQ',
  })
  // An older app doesn't know page.open: the popup stays and says what to do.
  await page.evaluate(() =>
    sessionStorage.setItem(
      'reply',
      JSON.stringify({ v: 1, type: 'download.declined', reason: 'invalid', fallback: 'browser' }),
    ),
  )
  await page.reload()
  await expect(page.locator('#status')).toHaveText(/Connected to Fuselane/)
  await page.getByRole('tab', { name: 'This page' }).click()
  await open.click()
  await expect(page.locator('#media-status')).toHaveText(
    'Update Fuselane to open pages from here, then try again.',
  )
  expect(await page.evaluate(() => (window as unknown as { shut: boolean }).shut)).toBe(false)
  await expect(open).toBeEnabled()
  expect(errors).toEqual([])
  await page.close()
})
