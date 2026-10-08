import { expect, test, type Page } from '@playwright/test'

const FEED = { version: '0.1.0-beta.1', notes: '', pub_date: '2026-10-08T00:00:00Z', platforms: {} }
const REL = 'https://github.com/ArshPunisher/fuselane/releases'

async function withFeed(page: Page, ok = true) {
  await page.route('**/updates/latest.json', (r) =>
    ok ? r.fulfill({ json: FEED }) : r.fulfill({ status: 404, body: 'not found' }),
  )
}

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  ;(page as Page & { errors: string[] }).errors = errors
})
test.afterEach(async ({ page }) => {
  expect((page as Page & { errors: string[] }).errors).toEqual([])
})

test('every download link points at the exact file of the current version', async ({ page }) => {
  await withFeed(page)
  await page.goto('/')
  await expect(page.locator('#version-tag')).toHaveText('0.1.0-beta.1')
  const v = '0.1.0-beta.1'
  const expected: Record<string, string> = {
    'macos-universal.dmg': `${REL}/download/v${v}/Fuselane_${v}_macos-universal.dmg`,
    'windows-x64-setup.exe': `${REL}/download/v${v}/Fuselane_${v}_windows-x64-setup.exe`,
    'linux-arm64.rpm': `${REL}/download/v${v}/Fuselane_${v}_linux-arm64.rpm`,
  }
  for (const [file, href] of Object.entries(expected)) {
    await expect(page.locator(`a[data-file="${file}"]`)).toHaveAttribute('href', href)
  }
  await expect(page.locator('a[data-cli="linux-x64.tar.gz"]')).toHaveAttribute(
    'href',
    `${REL}/download/v${v}/fuselane-cli_${v}_linux-x64.tar.gz`,
  )
  await expect(page.locator('#sums')).toHaveAttribute('href', `${REL}/download/v${v}/SHA256SUMS`)
})

test('without the feed, links fall back to the releases page instead of breaking', async ({
  page,
}) => {
  await withFeed(page, false)
  await page.goto('/')
  await expect(page.locator('a[data-file="macos-universal.dmg"]')).toHaveAttribute('href', REL)
  await expect(page.locator('#version-tag')).toHaveText('')
})

for (const [ua, platform, label, current] of [
  ['Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32', 'Download for Windows', 'windows'],
  ['Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64', 'Download for Linux', 'linux'],
  ['Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5)', 'MacIntel', 'Download for macOS', 'mac'],
] as const) {
  test(`a ${current} visitor gets the matching button and card`, async ({ browser }) => {
    const ctx = await browser.newContext({ userAgent: ua })
    const page = await ctx.newPage()
    await page.addInitScript(
      (p) => Object.defineProperty(navigator, 'platform', { get: () => p }),
      platform,
    )
    await withFeed(page)
    await page.goto('/')
    await expect(page.locator('#primary-download')).toHaveText(label)
    await expect(page.locator(`.platform[data-os="${current}"]`)).toHaveAttribute(
      'data-current',
      '',
    )
    if (current === 'windows') {
      await expect(page.locator('#primary-download')).toHaveAttribute(
        'href',
        /windows-x64-setup\.exe$/,
      )
    }
    await ctx.close()
  })
}

test('copy buttons copy the exact command', async ({ page, context, browserName }) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are Chromium-only in Playwright')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await withFeed(page)
  await page.goto('/')
  await page.locator('button[data-copy="cmd-brew"]').click()
  await expect(page.locator('button[data-copy="cmd-brew"]')).toHaveText('Copied')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'brew install --cask arshpunisher/tap/fuselane',
  )
})

test('the page mentions SignPath, privacy, and never scrolls sideways on a phone', async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await withFeed(page)
  await page.goto('/')
  await expect(page.getByText('SignPath Foundation')).toBeVisible()
  await expect(page.getByRole('link', { name: 'privacy policy' })).toHaveAttribute(
    'href',
    /PRIVACY\.md$/,
  )
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth - innerWidth),
  ).toBeLessThanOrEqual(0)
  await expect(page.locator('img[alt]')).toHaveCount(1)
})

test('SEO: title, description, canonical, share card and structured data are right', async ({
  page,
  request,
}) => {
  await withFeed(page)
  await page.goto('/')
  const title = await page.title()
  expect(title.length).toBeGreaterThan(20)
  expect(title.length).toBeLessThanOrEqual(80)
  const desc = (await page.locator('meta[name="description"]').getAttribute('content')) ?? ''
  expect(desc.length).toBeGreaterThan(70)
  expect(desc.length).toBeLessThanOrEqual(200)
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
    'href',
    'https://arshpunisher.github.io/fuselane/',
  )
  await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
    'content',
    /^https:\/\/.+\/og\.png$/,
  )
  await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute(
    'content',
    'summary_large_image',
  )
  await expect(page.locator('h1')).toHaveCount(1)
  const ld = JSON.parse(
    (await page.locator('script[type="application/ld+json"]').textContent()) ?? '{}',
  )
  expect(ld['@type']).toBe('SoftwareApplication')
  expect(ld.offers.price).toBe('0')
  expect(ld.operatingSystem).toContain('Windows')
  for (const path of [
    'robots.txt',
    'sitemap.xml',
    'favicon.ico',
    'favicon.svg',
    'og.png',
    'site.webmanifest',
    'apple-touch-icon.png',
  ]) {
    const r = await request.get(`/${path}`)
    expect(r.status(), path).toBe(200)
  }
  expect(await (await request.get('/robots.txt')).text()).toContain(
    'Sitemap: https://arshpunisher.github.io/fuselane/sitemap.xml',
  )
})

test('the header logo animates in and ends fully drawn', async ({ page }) => {
  await withFeed(page)
  await page.goto('/')
  await page.waitForTimeout(1500)
  const offsets = await page
    .locator('.mark path')
    .evaluateAll((ps) => ps.map((p) => getComputedStyle(p).strokeDashoffset))
  expect(offsets.every((o) => o === '0' || o === '0px')).toBe(true)
})
