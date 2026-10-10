import { expect, test, type Page } from '@playwright/test'

const FEED = { version: '0.1.0-beta.1', notes: '', pub_date: '2026-10-08T00:00:00Z', platforms: {} }
const REL = 'https://github.com/ArshPunisher/fuselane/releases'
const PAGES = ['/', '/download/', '/guide/', '/faq/', '/support/']

async function stub(page: Page, { feed = true, stars = 42 } = {}) {
  await page.route('**/updates/latest.json', (r) =>
    feed ? r.fulfill({ json: FEED }) : r.fulfill({ status: 404, body: 'not found' }),
  )
  // Never the real GitHub API in tests: a fixed star count and one asset size.
  await page.route('https://api.github.com/**', (r) =>
    r.fulfill({
      json: {
        stargazers_count: stars,
        assets: [{ name: 'Fuselane_0.1.0-beta.1_macos-universal.dmg', size: 18_422_626 }],
      },
    }),
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
  await stub(page)
  await page.goto('/download/')
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
  await expect(page.locator('#notes')).toHaveAttribute('href', `${REL}/tag/v${v}`)
  await expect(page.locator('[data-size="macos-universal.dmg"]')).toHaveText('18 MB')
})

test('without the feed, links fall back to the releases page instead of breaking', async ({
  page,
}) => {
  await stub(page, { feed: false })
  await page.goto('/download/')
  await expect(page.locator('a[data-file="macos-universal.dmg"]')).toHaveAttribute('href', REL)
  await expect(page.locator('#version-tag')).toHaveText('')
})

for (const [ua, platform, label, os, tabName] of [
  [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
    'Win32',
    'Download for Windows',
    'windows',
    /Windows/,
  ],
  ['Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64', 'Download for Linux', 'linux', /Linux/],
  [
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5)',
    'MacIntel',
    'Download for macOS',
    'mac',
    /macOS/,
  ],
] as const) {
  test(`a ${os} visitor gets the matching button and tab`, async ({ browser }) => {
    const ctx = await browser.newContext({ userAgent: ua })
    const page = await ctx.newPage()
    await page.addInitScript(
      (p) => Object.defineProperty(navigator, 'platform', { get: () => p }),
      platform,
    )
    await stub(page)
    await page.goto('/')
    await expect(page.locator('#primary-download')).toHaveText(label)
    if (os === 'windows') {
      await expect(page.locator('#primary-download')).toHaveAttribute(
        'href',
        /windows-x64-setup\.exe$/,
      )
    }
    await page.goto('/download/')
    const tab = page.getByRole('tab', { name: tabName })
    await expect(tab).toHaveAttribute('aria-selected', 'true')
    await expect(tab).toContainText('Yours')
    await expect(page.locator(`#panel-${os}`)).toBeVisible()
    await ctx.close()
  })
}

test('system tabs work with the keyboard and show one panel at a time', async ({ page }) => {
  await stub(page)
  await page.goto('/download/')
  const mac = page.getByRole('tab', { name: /macOS/ })
  await mac.click()
  await mac.press('ArrowRight')
  const win = page.getByRole('tab', { name: /Windows/ })
  await expect(win).toBeFocused()
  await expect(win).toHaveAttribute('aria-selected', 'true')
  await expect(page.locator('#panel-windows')).toBeVisible()
  await expect(page.locator('#panel-mac')).toBeHidden()
  await page.getByText('Windows says it protected your PC?').click()
  await expect(page.getByText(/SignPath Foundation/)).toBeVisible()
  await win.press('End')
  await expect(page.getByRole('tab', { name: /Linux/ })).toHaveAttribute('aria-selected', 'true')
})

test('the Mac panel shows how to open the app the first time', async ({ page }) => {
  await stub(page)
  await page.goto('/download/')
  await page.getByRole('tab', { name: /macOS/ }).click()
  const steps = page.locator('.open-steps li')
  await expect(steps).toHaveCount(3)
  await expect(steps.nth(0)).toContainText('Not Opened')
  await expect(steps.nth(2)).toContainText('Open Anyway')
  await expect(page.getByText('Or skip the warning: install with one command')).toBeVisible()
})

test('the Open Anyway guide is illustrated, step by step', async ({ page }) => {
  await stub(page)
  await page.goto('/download/')
  await page.getByRole('tab', { name: /macOS/ }).click()
  const steps = page.locator('.open-steps li')
  // Every step has its own picture, named for screen readers.
  await expect(steps.getByRole('img')).toHaveCount(3)
  await expect(steps.nth(0).getByRole('img')).toHaveAccessibleName(/Not Opened.*Done/)
  await expect(steps.nth(1).getByRole('img')).toHaveAccessibleName(/Privacy & Security/)
  await expect(steps.nth(2).getByRole('img')).toHaveAccessibleName(/Open Anyway/)
  // While on screen the steps take turns; pointing at one holds it.
  await page.locator('#open-anyway').scrollIntoViewIfNeeded()
  await expect(steps.nth(0)).toHaveAttribute('data-active', '')
  await steps.nth(2).hover()
  await expect(steps.nth(2)).toHaveAttribute('data-active', '')
  await expect(steps.nth(0)).not.toHaveAttribute('data-active', '')
  // The one-line install shows what it prints, with the current version.
  await expect(page.locator('.term-out')).toContainText(
    'fuselane: installed Fuselane 0.1.0-beta.1 in /Applications.',
  )
  // Windows: SmartScreen's two clicks, drawn too.
  await page.getByRole('tab', { name: /Windows/ }).click()
  await page.getByText('Windows says it protected your PC?').click()
  await expect(page.locator('.win-steps').getByRole('img')).toHaveCount(2)
  await expect(page.locator('.win-steps')).toContainText('Run anyway')
})

test('copy buttons copy the exact command', async ({ page, context, browserName }) => {
  test.skip(browserName !== 'chromium', 'clipboard permissions are Chromium-only in Playwright')
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await stub(page)
  await page.goto('/download/')
  await page.getByRole('tab', { name: /macOS/ }).click()
  await page.locator('button[data-copy="cmd-brew"]').click()
  await expect(page.locator('button[data-copy="cmd-brew"]')).toHaveText('Copied')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'brew install --cask arshpunisher/tap/fuselane',
  )
  // The long one wraps on screen but copies as one line.
  await page.locator('button[data-copy="cmd-script"]').click()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'curl -fsSL https://raw.githubusercontent.com/ArshPunisher/fuselane/main/packaging/macos/install.sh | sh',
  )
})

test('turning a network off in the hero takes its speed away', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  const sw = (name: string) => page.getByRole('switch', { name: new RegExp(name) })
  await expect(sw('Ethernet')).toHaveAttribute('aria-checked', 'true')
  await sw('Ethernet').click()
  await sw('iPhone USB').click()
  await expect(sw('Ethernet')).toHaveAttribute('aria-checked', 'false')
  await expect(page.locator('#core-live')).toHaveText('Wi-Fi: about 41.2 MB/s together.')
  // The others slow to a stop; only Wi-Fi's sample speed is left.
  await expect
    .poll(async () => Number(await page.locator('#total').textContent()), { timeout: 5000 })
    .toBeLessThan(48)
  await expect(page.locator('#gain')).toHaveText('One network')
  await sw('Wi-Fi').click()
  await expect(page.locator('#gain')).toHaveText('Turn a network on')
  await expect(page.locator('#file-left')).toContainText('waiting for a network')
  await sw('Ethernet').click()
  await expect(page.locator('#core-live')).toHaveText('Ethernet: about 33.8 MB/s together.')
})

test('the hero shows live sample speeds that add up', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  await page.waitForTimeout(700)
  // Read everything in one go: the numbers change every frame.
  const { total, sum } = await page.evaluate(() => ({
    total: Number(document.querySelector('#total')?.textContent),
    sum: [...document.querySelectorAll('[data-lane-rate]')]
      .map((e) => Number(e.textContent))
      .reduce((a, b) => a + b, 0),
  }))
  expect(total).toBeGreaterThan(70)
  expect(total).toBeLessThan(100)
  expect(Math.abs(total - sum)).toBeLessThan(0.25)
  await expect(page.getByText('Sample speeds')).toBeVisible()
})

test('every page works on a phone without sideways scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await stub(page)
  for (const path of PAGES) {
    await page.goto(path)
    await expect(page.locator('h1')).toHaveCount(1)
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth - innerWidth),
      path,
    ).toBeLessThanOrEqual(0)
    // The page list sits behind the menu button.
    const menu = page.getByRole('button', { name: 'Menu' })
    await expect(page.getByRole('navigation', { name: 'Pages' })).toBeHidden()
    await menu.click()
    await expect(menu).toHaveAttribute('aria-expanded', 'true')
    await expect(page.getByRole('navigation', { name: 'Pages' })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByRole('navigation', { name: 'Pages' })).toBeHidden()
    await expect(menu).toBeFocused()
  }
})

test('the navbar tightens once the page scrolls, and its highlight follows the pointer', async ({
  page,
}) => {
  await stub(page)
  await page.goto('/faq/')
  const nav = page.locator('[data-nav]')
  await expect(nav).not.toHaveAttribute('data-scrolled', '')
  const track = page.locator('.nav-track')
  // Resting on the current page.
  await expect(track).toHaveCSS('--glide-o', '1')
  const faqX = await track.evaluate((t) => t.style.getPropertyValue('--glide-x'))
  await page.getByRole('navigation', { name: 'Main' }).getByRole('link', { name: 'Send' }).hover()
  await expect
    .poll(() => track.evaluate((t) => t.style.getPropertyValue('--glide-x')))
    .not.toBe(faqX)
  await page.mouse.wheel(0, 800)
  await expect(nav).toHaveAttribute('data-scrolled', '')
})

test('the combined speed rolls and keeps running, while the exact figure stays readable', async ({
  page,
}) => {
  await stub(page)
  await page.goto('/')
  const odo = page.locator('.core .odo')
  await expect(odo.locator('.odo-d')).toHaveCount(3) // 87.4
  const read = () =>
    odo.evaluate((el) =>
      [...el.children]
        .map((c) =>
          c.classList.contains('odo-d')
            ? getComputedStyle(c.firstElementChild!).getPropertyValue('--n')
            : '.',
        )
        .join(''),
    )
  const first = await read()
  await expect.poll(read, { timeout: 6000 }).not.toBe(first)
  await expect(page.locator('#total')).toHaveText(/^\d+\.\d$/)
})

test('how it works follows the scroll: split, spread, fuse, and back', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  const visual = page.locator('.story-visual')
  for (const step of ['1', '2', '3', '1']) {
    await page
      .locator(`.story-step[data-step="${step}"]`)
      .evaluate((el) => el.scrollIntoView({ block: 'center', behavior: 'instant' }))
    await expect(visual).toHaveAttribute('data-step', step)
    await expect(page.locator(`.story-step[data-step="${step}"]`)).toHaveAttribute(
      'aria-current',
      'step',
    )
  }
  // The measured numbers end on their real values.
  await page.locator('.proof').evaluate((el) => el.scrollIntoView({ behavior: 'instant' }))
  await expect(page.locator('.proof [data-count]').first()).toHaveText('18.6')
  await expect(page.locator('.proof-list')).toContainText('246 MB 1080p video')
})

test('the race works out both times from the measured speeds', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  const race = page.locator('.race')
  await race.evaluate((el) => el.scrollIntoView({ block: 'center', behavior: 'instant' }))
  // 10 GB at 117 Mbps, and at 117 + 113 Mbps.
  await expect(page.locator('#race-one')).toHaveText('11 min 24 s')
  await expect(page.locator('#race-both')).toHaveText('5 min 48 s')
  await expect(race).not.toHaveAttribute('data-running', '', { timeout: 8000 })
  await expect(page.locator('#race-clock')).toHaveText('11:24')
  await page.getByRole('radio', { name: '50 GB' }).check()
  await expect(page.locator('#race-one')).toHaveText('56 min 59 s')
  await expect(page.locator('#race-result')).toHaveText(
    'Both together: 28 min 59 s, which is 28 min sooner.',
  )
  await expect(page.locator('#race-clock')).toHaveText('56:59', { timeout: 8000 })
})

test('built-in tools: one group at a time, by click or arrow keys', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  const tabs = page.getByRole('tablist', { name: /built in/ })
  const get = tabs.getByRole('tab', { name: /Get any file/ })
  const nets = tabs.getByRole('tab', { name: /Know your networks/ })
  await expect(get).toHaveAttribute('aria-selected', 'true')
  await expect(page.locator('#tool-get')).toBeVisible()
  await expect(page.locator('#tool-nets')).toBeHidden()
  await nets.click()
  await expect(page.locator('#tool-nets')).toBeVisible()
  await expect(page.locator('#tool-get')).toBeHidden()
  await expect(page.locator('#tool-nets')).toContainText('Network check')
  await nets.press('ArrowRight')
  await expect(tabs.getByRole('tab', { name: /Stay in control/ })).toBeFocused()
  await expect(page.locator('#tool-control')).toBeVisible()
  await page.keyboard.press('Home')
  await expect(get).toHaveAttribute('aria-selected', 'true')
  // Every screen is the real app, in the visitor's theme.
  const img = page.locator('#tool-get img')
  await expect(img).toHaveAttribute('src', /shots\/feeds-dark\.webp$/)
  await expect(page.locator('#tool-get source')).toHaveAttribute('srcset', /feeds-light\.webp/)
})

test('the nav marks the current page and every page links to privacy', async ({ page }) => {
  await stub(page)
  for (const [path, name] of [
    ['/download/', 'Download'],
    ['/guide/', 'Guide'],
    ['/faq/', 'FAQ'],
    ['/support/', 'Support'],
  ] as const) {
    await page.goto(path)
    await expect(
      page.getByRole('navigation', { name: 'Main' }).getByRole('link', { name, exact: true }),
    ).toHaveAttribute('aria-current', 'page')
    await expect(page.getByRole('link', { name: 'Privacy', exact: true })).toHaveAttribute(
      'href',
      /PRIVACY\.md$/,
    )
  }
})

test('the star count shows when there is one, and never as 0', async ({ page, browser }) => {
  await stub(page, { stars: 1234 })
  await page.goto('/')
  await expect(page.locator('.star .count')).toHaveText(/1\.2K|1,234|1234/)
  const ctx = await browser.newContext()
  const fresh = await ctx.newPage()
  await stub(fresh, { stars: 0 })
  await fresh.goto('/support/')
  await fresh.waitForTimeout(400)
  await expect(fresh.locator('.star .count')).toHaveText('')
  await ctx.close()
})

test('the FAQ is grouped by topic and the topic list jumps to each', async ({ page }) => {
  await stub(page)
  await page.goto('/faq/')
  const topics = page.getByRole('navigation', { name: 'Topics' })
  for (const t of ['How it works', 'Fuse Send', 'Installing and trust'])
    await expect(page.getByRole('heading', { level: 2, name: t })).toBeVisible()
  await topics.getByRole('link', { name: /Installing and trust/ }).click()
  await expect(page).toHaveURL(/#trust$/)
  await expect(topics.getByRole('link', { name: /Installing and trust/ })).toHaveAttribute(
    'aria-current',
    'true',
  )
  await expect(page.locator('.qa details')).toHaveCount(17)
})

test('the guide explains each part and is honest about limits', async ({ page }) => {
  await stub(page)
  await page.goto('/guide/')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('The Fuselane guide')
  const toc = page.getByRole('navigation', { name: 'Guide' })
  await expect(toc.getByRole('link')).toHaveCount(11)
  await toc.getByRole('link', { name: 'Limits, honestly' }).click()
  await expect(page).toHaveURL(/#limits$/)
  await expect(toc.getByRole('link', { name: 'Limits, honestly' })).toHaveAttribute(
    'aria-current',
    'true',
  )
  const limits = page.locator('#limits')
  for (const text of [
    'the file comes over one network',
    'DHT and tracker lookups use the default network',
    'not notarized',
    'SignPath Foundation',
  ])
    await expect(limits).toContainText(text)
  // The home page's honest notes link here.
  await page.goto('/')
  await expect(page.getByRole('link', { name: /All the limits, in the guide/ })).toHaveAttribute(
    'href',
    'guide/#limits',
  )
})

test('without a known version the download page still reads well', async ({ page }) => {
  await stub(page, { feed: false })
  await page.goto('/download/')
  const line = page.locator('.page-hero p').first()
  await expect(line).toContainText('For macOS, Windows and Linux.', { useInnerText: true })
  await expect(line).not.toContainText('Version', { useInnerText: true })
})

test('FAQ answers open and close', async ({ page }) => {
  await stub(page)
  await page.goto('/faq/')
  const q = page.getByText("Will it eat my phone's data?")
  const a = page.getByText(/monthly allowance for each\s+network/)
  await q.click()
  await expect(a).toBeVisible()
  await q.click()
  await expect(a).toBeHidden()
})

test('SEO: each page has its own title, description and canonical; shared files exist', async ({
  page,
  request,
}) => {
  await stub(page)
  const seen = new Set<string>()
  for (const path of PAGES) {
    await page.goto(path)
    const title = await page.title()
    expect(title.length, path).toBeGreaterThan(20)
    expect(title.length, path).toBeLessThanOrEqual(80)
    expect(seen.has(title), `${path} repeats a title`).toBe(false)
    seen.add(title)
    const desc = (await page.locator('meta[name="description"]').getAttribute('content')) ?? ''
    expect(desc.length, path).toBeGreaterThan(70)
    expect(desc.length, path).toBeLessThanOrEqual(200)
    await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
      'href',
      `https://fuselane.app${path}`,
    )
    await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
      'content',
      /^https:\/\/.+\/og\.png$/,
    )
    for (const img of await page.locator('img').all())
      expect((await img.getAttribute('alt'))?.length ?? 0, path).toBeGreaterThan(10)
  }
  await page.goto('/')
  const blocks = await page.locator('script[type="application/ld+json"]').allTextContents()
  const ld = blocks.map((b) => JSON.parse(b))
  const app = ld.find((d) => d['@type'] === 'SoftwareApplication')
  expect(app.offers.price).toBe('0')
  expect(app.url).toBe('https://fuselane.app/')
  expect(ld.some((d) => d['@type'] === 'WebSite')).toBe(true)
  // The FAQ's questions are marked up for search results, and pages say where they sit.
  await page.goto('/faq/')
  const faq = (await page.locator('script[type="application/ld+json"]').allTextContents())
    .map((b) => JSON.parse(b))
    .find((d) => d['@type'] === 'FAQPage')
  expect(faq.mainEntity.length).toBe(await page.locator('main details').count())
  expect(faq.mainEntity[0].acceptedAnswer.text.length).toBeGreaterThan(40)
  expect(
    (await page.locator('script[type="application/ld+json"]').allTextContents()).some((b) =>
      b.includes('BreadcrumbList'),
    ),
  ).toBe(true)
  for (const path of [
    'robots.txt',
    'sitemap.xml',
    'favicon.ico',
    'favicon.svg',
    'og.png',
    'site.webmanifest',
    'apple-touch-icon.png',
    'shots/app.png',
    'shots/torrent.png',
    'shots/feeds-dark.webp',
    'shots/send-light.webp',
    'shots/check-dark.webp',
    'shots/control-light.webp',
  ]) {
    const r = await request.get(`/${path}`)
    expect(r.status(), path).toBe(200)
  }
  const sitemap = await (await request.get('/sitemap.xml')).text()
  for (const path of PAGES) expect(sitemap).toContain(`https://fuselane.app${path}`)
})

test('the logo draws itself in and ends fully drawn', async ({ page }) => {
  await stub(page)
  await page.goto('/')
  await page.waitForTimeout(1500)
  const offsets = await page
    .locator('.nav .mark path')
    .evaluateAll((ps) => ps.map((p) => getComputedStyle(p).strokeDashoffset))
  expect(offsets.every((o) => o === '0' || o === '0px')).toBe(true)
})

test('with reduced motion, nothing waits to appear and nothing loops', async ({ browser }) => {
  const ctx = await browser.newContext({ reducedMotion: 'reduce' })
  const page = await ctx.newPage()
  await stub(page)
  await page.goto('/')
  // Sections that would fade in on scroll are visible straight away.
  for (const sel of ['#features .reveal', '.notes-list li', '.foot'])
    expect(
      await page
        .locator(sel)
        .first()
        .evaluate((e) => getComputedStyle(e).opacity),
    ).toBe('1')
  await expect(page.locator('#total')).not.toHaveText('')
  // The race is shown finished, the marquee doesn't move, the key doesn't scramble.
  await expect(page.locator('#race-clock')).toHaveText('11:24')
  await expect(page.locator('.race')).not.toHaveAttribute('data-running', '')
  await expect(page.locator('.marquee')).not.toHaveAttribute('data-anim', '')
  await expect(page.locator('.marquee-list')).toHaveCount(1)
  await expect(page.locator('.anatomy')).not.toHaveAttribute('data-play', '')
  // The hero still answers its switches, without moving.
  await page.getByRole('switch', { name: /Ethernet/ }).click()
  await expect(page.locator('#total')).toHaveText('53.6')
  await ctx.close()
})

test('the home page reads well without JavaScript', async ({ browser }) => {
  const ctx = await browser.newContext({ javaScriptEnabled: false })
  const page = await ctx.newPage()
  await page.goto('/')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Every network.')
  // Every tool group shows, with its own heading.
  for (const name of [
    'Get any file',
    'Between your devices',
    'Know your networks',
    'Stay in control',
  ])
    await expect(page.getByRole('heading', { level: 3, name })).toBeVisible()
  await expect(page.locator('#race-one')).toHaveText('11 min 24 s')
  await expect(page.getByText('Measured on real machines')).toBeVisible()
  await ctx.close()
})

test.describe('the Fuse Send link page', () => {
  // 53 bytes (info-hash, key, flags) in base64url: 71 characters.
  const TOKEN = 'v1.' + 'A'.repeat(70) + 'w'

  test('a whole link opens in the app and never leaves the tab', async ({ page }) => {
    const seen: string[] = []
    page.on('request', (r) => seen.push(`${r.url()} ${r.headers()['referer'] ?? ''}`))
    await stub(page)
    await page.goto(`/s/#${TOKEN}`)
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Someone sent you a file')
    await expect(page.getByRole('link', { name: 'Open in Fuselane' })).toHaveAttribute(
      'href',
      `fuselane://send/${TOKEN}`,
    )
    await expect(page.getByRole('button', { name: 'Copy link' })).toBeVisible()
    await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', /noindex/)
    expect(seen.filter((s) => s.includes(TOKEN.slice(3, 20)))).toEqual([])
  })

  for (const [hash, title] of [
    ['', 'This link is missing its key'],
    ['#v1.abc', 'This link is incomplete'],
    ['#hello', 'This link is incomplete'],
    [`#v2.${'A'.repeat(71)}`, 'This link needs a newer Fuselane'],
  ] as const) {
    test(`"${hash || 'no key'}" explains what went wrong`, async ({ page }) => {
      await stub(page)
      await page.goto(`/s/${hash}`)
      await expect(page.getByRole('heading', { level: 1 })).toHaveText(title)
      await expect(page.getByRole('link', { name: 'Open in Fuselane' })).toBeHidden()
    })
  }

  test('fixing the link in place shows the right page again', async ({ page }) => {
    await stub(page)
    await page.goto('/s/#v1.abc')
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('This link is incomplete')
    await page.evaluate((t) => (location.hash = t), TOKEN)
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Someone sent you a file')
    await expect(page.getByText('It comes straight from their computer')).toBeVisible()
  })

  test('fits a phone', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 })
    await stub(page)
    await page.goto(`/s/#${TOKEN}`)
    await expect(page.getByRole('link', { name: 'Open in Fuselane' })).toBeVisible()
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth - innerWidth),
    ).toBeLessThanOrEqual(0)
  })
})
