import { execFileSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test, type Page } from '@playwright/test'

// Translations (STEPS 8.8): the Language setting, ?lang=hi, the choice kept over a
// reload, Hindi layouts at 375 px, and the catalogue check (scripts/i18n-check.ts).

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

const NAV_HI = ['डाउनलोड', 'भेजें', 'नेटवर्क', 'सेटिंग्स']

/** Visible text in `root` outside names, files, numbers and code: what a translation covers. */
async function englishLeft(page: Page, root: string): Promise<string[]> {
  return page.locator(root).evaluateAll((els) => {
    // Brand names, standards and units stay as they are (docs/07-design/I18N.md).
    const keep =
      /Fuselane|Fuse Send|LocalSend|AriaNg|aria2|yt-dlp|ffmpeg|BitTorrent|GitHub|Wi-Fi|Ethernet|VPN|USB|DNS|SHA-256|RSS|Metalink|QR|MB\/s|Mbps|[KMGT]i?B|Esc|Ctrl/g
    const out: string[] = []
    for (const el of els) {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT)
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const parent = n.parentElement
        if (!parent || parent.closest('[translate="no"], code, kbd, .num, [aria-hidden="true"]'))
          continue
        if (!parent.checkVisibility()) continue
        // File names ("ubuntu-26.04-desktop-amd64.iso") stay as they are, even in a
        // screen reader's announcement.
        const text = (n.textContent ?? '').replace(/[\w()-]+(?:\.[\w-]+)+/g, ' ').replace(keep, ' ')
        if (/[A-Za-z]{2,}/.test(text)) out.push((n.textContent ?? '').trim())
      }
    }
    return out
  })
}

test('the demo starts in English whatever the browser language', async ({ browser }) => {
  const context = await browser.newContext({ locale: 'hi-IN' })
  const page = await context.newPage()
  await page.goto('/')
  await expect(page.getByRole('navigation', { name: 'Main' })).toContainText('Downloads')
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
  await context.close()
})

test('choosing हिन्दी in Settings switches the screens at once, and English switches back', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  const language = page.getByRole('radiogroup', { name: 'Language' })
  await expect(language.getByRole('radio', { name: 'System' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await language.getByRole('radio', { name: 'हिन्दी' }).click()

  await expect(page.locator('html')).toHaveAttribute('lang', 'hi')
  const nav = page.getByRole('navigation', { name: 'मुख्य' })
  for (const label of NAV_HI) await expect(nav.getByRole('button', { name: label })).toBeVisible()
  await expect(page.getByRole('heading', { level: 1, name: 'सेटिंग्स' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'रूप-रंग' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'नया डाउनलोड' }).first()).toBeVisible()
  // The language names stay in their own language, so anyone can find theirs.
  const hindi = page.getByRole('radiogroup', { name: 'भाषा' })
  await expect(hindi.getByRole('radio', { name: 'हिन्दी' })).toHaveAttribute('aria-checked', 'true')
  await expect(hindi.getByRole('radio', { name: 'English' })).toHaveAttribute('lang', 'en')
  expect(await englishLeft(page, '.settings-group h2, .setting-name')).toEqual([])

  await hindi.getByRole('radio', { name: 'English' }).click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en')
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Main' })).toContainText('Downloads')
})

test('?lang=hi shows Downloads in Hindi, with no English left in the nav or buttons', async ({
  page,
}) => {
  await page.goto('/?lang=hi')
  await expect(page.locator('html')).toHaveAttribute('lang', 'hi')
  const nav = page.getByRole('navigation', { name: 'मुख्य' })
  for (const label of NAV_HI) await expect(nav.getByRole('button', { name: label })).toBeVisible()
  await expect(page.getByTestId('fuse-core')).toBeVisible()
  await expect(page.getByText('डेमो डेटा')).toBeVisible()
  expect(await englishLeft(page, 'nav, .sidebar')).toEqual([])
  expect(await englishLeft(page, '.btn, .icon-btn, .chip, .seg, [role="tab"]')).toEqual([])
  // With the live network table filled in (network names stay as the system names them).
  await expect(page.getByRole('table', { name: 'इस डाउनलोड के नेटवर्क' })).toContainText(
    'Ethernet',
    { timeout: 5000 },
  )
  expect(await englishLeft(page, 'main')).toEqual([])
})

test('the language choice is kept over a reload', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  await page
    .getByRole('radiogroup', { name: 'Language' })
    .getByRole('radio', { name: 'हिन्दी' })
    .click()
  await expect(page.getByRole('heading', { level: 1, name: 'सेटिंग्स' })).toBeVisible()
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('lang', 'hi')
  await expect(page.getByRole('navigation', { name: 'मुख्य' })).toContainText('डाउनलोड')
  await page.getByRole('button', { name: 'सेटिंग्स' }).first().click()
  await expect(
    page.getByRole('radiogroup', { name: 'भाषा' }).getByRole('radio', { name: 'हिन्दी' }),
  ).toHaveAttribute('aria-checked', 'true')
})

test.describe('at 375 px', () => {
  test.use({ viewport: { width: 375, height: 812 } })

  test('no page scrolls sideways in Hindi', async ({ page }) => {
    await page.goto('/?lang=hi')
    const tabs = page.getByRole('navigation', { name: 'मुख्य' })
    const overflow = () =>
      page.evaluate(() => {
        // Sticking out past the window, unless a scrolling or clipping box holds it.
        const held = (el: Element) => {
          for (let p = el.parentElement; p; p = p.parentElement)
            if (
              getComputedStyle(p).overflowX !== 'visible' &&
              p.getBoundingClientRect().right <= innerWidth + 1
            )
              return true
          return false
        }
        const wide = [...document.querySelectorAll<HTMLElement>('main, main *, nav, nav *')]
          .filter((el) => el.checkVisibility() && el.getBoundingClientRect().right > innerWidth + 1)
          .filter((el) => !held(el))
          .map((el) => `${el.tagName.toLowerCase()}.${el.className}`)
        return {
          page: document.documentElement.scrollWidth - innerWidth,
          wide: [...new Set(wide)].slice(0, 5),
        }
      })
    for (const label of NAV_HI) {
      await tabs.getByRole('button', { name: label }).click()
      await expect(tabs.getByRole('button', { name: label })).toHaveAttribute(
        'aria-current',
        'page',
      )
      // Rows fade and slide in: wait for the page to settle.
      await expect.poll(overflow, { message: label }).toEqual({ page: 0, wide: [] })
    }
    await page.getByRole('button', { name: 'नया डाउनलोड' }).first().click()
    const dialog = page.getByRole('dialog', { name: 'नया डाउनलोड' })
    await expect(dialog).toBeVisible()
    expect(
      await dialog.evaluate((d) => d.scrollWidth - d.clientWidth),
      'new download dialog',
    ).toBeLessThanOrEqual(0)
  })
})

test('every string in the code has Hindi (scripts/i18n-check.ts)', async ({}, info) => {
  test.skip(info.project.name !== 'chromium', 'a file check: once is enough')
  const app = join(dirname(fileURLToPath(import.meta.url)), '..')
  let out = ''
  let ok = true
  try {
    out = execFileSync(process.execPath, ['scripts/i18n-check.ts'], { cwd: app, encoding: 'utf8' })
  } catch (e) {
    ok = false
    out = String((e as { stdout?: string }).stdout ?? e)
  }
  expect(ok, out).toBe(true)
})
