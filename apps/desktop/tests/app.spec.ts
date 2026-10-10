import { expect, test, type Page } from '@playwright/test'

// Fail any test that logs an error to the console.
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

async function openDialog(page: Page) {
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await expect(dialog).toBeVisible()
  return dialog
}

test('lists downloads, says it is demo data, and shows the running one fusing', async ({
  page,
}) => {
  await page.goto('/')
  await expect(page.getByText('Demo data')).toBeVisible()
  await expect(page.getByRole('heading', { name: /Active/ })).toBeVisible()
  await expect(page.getByRole('heading', { name: /Recent/ })).toBeVisible()
  await expect(
    page.getByRole('heading', { level: 1, name: 'ubuntu-26.04-desktop-amd64.iso' }),
  ).toBeVisible()
  await expect(page.getByTestId('fuse-core')).toBeVisible()
  // Live speed appears and the network table fills in.
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  await expect(page.getByRole('table', { name: 'Networks in this download' })).toContainText(
    'Ethernet',
  )
})

test('bad links are refused inline with what to do', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  const link = dialog.getByLabel('Link')
  const submit = dialog.getByRole('button', { name: 'Download', exact: true })
  for (const [bad, says] of [
    ['ftp://example.com/file', 'ftp: links aren'],
    ['javascript:alert(1)', 'javascript: links aren'],
    ['file:///etc/passwd', 'file: links aren'],
    ['not a link', "isn't a valid link"],
    ['   ', 'Paste a link'],
  ] as const) {
    await link.fill(bad)
    await submit.click()
    await expect(dialog.locator('.field-error')).toContainText(says)
    await expect(link).toHaveAttribute('aria-invalid', 'true')
    await expect(link).toBeFocused()
  }
  // Typing again clears the error.
  await link.fill('https://example.com/a.iso')
  await expect(dialog.locator('.field-error')).toHaveCount(0)
  // A folder that doesn't exist is refused on its own field.
  await dialog.getByLabel('Save to').fill('relative/nowhere')
  await submit.click()
  await expect(dialog.locator('#nd-dir-err')).toContainText("doesn't exist")
  await expect(dialog.getByLabel('Save to')).toBeFocused()
  await expect(page.getByText('Nothing downloading yet')).toBeVisible() // nothing was added
})

test('adding a link starts it and opens its detail', async ({ page }) => {
  await page.goto('/?empty=1')
  await expect(page.getByText('Nothing downloading yet')).toBeVisible()
  const dialog = await openDialog(page)
  await dialog
    .getByLabel('Link')
    .fill('  https://mirror.example.net/pub/linux%20image.iso?token=abc  ')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('heading', { level: 1, name: 'linux image.iso' })).toBeVisible()
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
})

test('hostile file names render as text, never markup', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  const evil = '<img src=x onerror="window.pwned=1">‮gpj.exe'
  await dialog.getByLabel('Link').fill(`https://example.com/${encodeURIComponent(evil)}`)
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(page.locator('.row-name').first()).toContainText('<img')
  expect(await page.evaluate(() => (window as unknown as { pwned?: number }).pwned)).toBeUndefined()
  expect(await page.locator('img').count()).toBe(0)
})

test('pause, resume and a two-step remove', async ({ page }) => {
  await page.goto('/')
  const detail = page.locator('article.detail')
  await detail.getByRole('button', { name: 'Pause' }).click()
  await expect(detail.locator('.speed-sub')).toHaveText('Paused')
  await detail.getByRole('button', { name: 'Resume' }).click()
  await expect(detail.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  await detail.getByRole('button', { name: 'Remove' }).click()
  // It asks first, in a dialog that waits.
  const ask = page.getByRole('dialog', { name: 'Stop and remove this download?' })
  await expect(ask).toBeVisible()
  await expect(ask.getByRole('button', { name: 'Cancel' })).toBeFocused()
  await ask.getByRole('button', { name: 'Delete unfinished file' }).click()
  await expect(ask).toBeHidden()
  await expect(page.locator('.row-name', { hasText: 'ubuntu-26.04' })).toHaveCount(0)
})

test('remove waits for an answer: no timer, and Esc or a click outside cancels', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  await page.getByText('Blender-5.1-macos-arm64.dmg').click()
  const detail = page.locator('article.detail')
  await detail.getByRole('button', { name: 'Remove' }).click()
  const ask = page.getByRole('dialog', { name: 'Remove this download?' })
  await expect(ask).toBeVisible()
  // The old inline confirm went away after 4 s; this one stays.
  await page.waitForTimeout(6500)
  await expect(ask).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(ask).toBeHidden()
  await expect(page.getByText('Blender-5.1-macos-arm64.dmg').first()).toBeVisible()
  // A click on the dimmed area outside cancels too.
  await detail.getByRole('button', { name: 'Remove' }).click()
  await expect(ask).toBeVisible()
  await page.mouse.click(8, 8)
  await expect(ask).toBeHidden()
  // Keep file: gone from the list, file untouched.
  await detail.getByRole('button', { name: 'Remove' }).click()
  await ask.getByRole('button', { name: 'Keep file' }).click()
  await expect(page.locator('.row-name', { hasText: 'Blender-5.1' })).toHaveCount(0)
})

test('a failed download shows the catalogue message and can be retried', async ({ page }) => {
  await page.goto('/')
  await page.getByText('nightly-build-2026-10-07.zip').click()
  await expect(page.getByRole('alert').filter({ hasText: 'stopped working' })).toBeVisible()
  await page.locator('article.detail').getByRole('button', { name: 'Try again' }).click()
  await expect(page.locator('article.detail .speed')).toContainText('MB/s', { timeout: 5000 })
})

test('keyboard: Ctrl+N opens the dialog and Escape closes it', async ({ page }) => {
  await page.goto('/')
  await page.locator('body').click({ position: { x: 5, y: 890 } })
  await page.keyboard.press('Control+n')
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByLabel('Link')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
})

test('theme choice applies and survives a reload', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('radio', { name: 'Light' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
  await page.getByRole('button', { name: 'Settings' }).click()
  // Arrow keys move the choice like a native radio group.
  await page.getByRole('radio', { name: 'Light' }).focus()
  await page.keyboard.press('ArrowRight')
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  // Language has a System choice too.
  await page
    .getByRole('radiogroup', { name: 'Theme' })
    .getByRole('radio', { name: 'System' })
    .click()
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.*/)
})

test('networks page lists usable networks and explains skipped ones', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Networks' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Networks' })).toBeVisible()
  await expect(page.locator('.net-cards')).toContainText('iPhone USB')
  // The live picture shows each network flowing into Fuselane.
  await expect(page.getByRole('region', { name: 'Your networks, joined' })).toContainText(
    '3 networks',
  )
  await page.getByText('Not used (1)').click()
  await expect(page.getByText(/Tunnels are skipped/)).toBeVisible()
})

for (const [w, h, layout] of [
  [360, 560, 'compact'],
  [375, 812, 'compact'],
  [800, 700, 'regular'],
  [1024, 700, 'wide'],
  [1180, 760, 'wide'],
  [1279, 800, 'wide'],
  [1920, 1080, 'wide'],
] as const) {
  test(`layout at ${w}x${h} is ${layout} with no sideways scroll`, async ({ page }) => {
    await page.setViewportSize({ width: w, height: h })
    await page.goto('/')
    await expect(page.locator('.app')).toHaveAttribute('data-layout', layout)
    await page.getByText('ubuntu-26.04-desktop-amd64.iso').first().click()
    await expect(page.getByTestId('fuse-core')).toBeVisible()
    await page.waitForTimeout(300)
    // Neither the page nor any scrolling pane may scroll sideways.
    const overflow = await page.evaluate(() =>
      [
        document.documentElement,
        ...document.querySelectorAll<HTMLElement>('.main, .pane-list, .pane-detail'),
      ]
        .map((e) => ({ el: e.className || 'html', extra: e.scrollWidth - e.clientWidth }))
        .filter((o) => o.extra > 0),
    )
    expect(overflow).toEqual([])
    if (layout !== 'wide') {
      await page.getByRole('button', { name: 'Back to downloads' }).click()
      await expect(page.getByRole('heading', { name: /Active/ })).toBeVisible()
    }
  })
}

test('reduced motion still shows progress', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
})

test('200 downloads with awkward names stay usable and never scroll sideways', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/?many=200')
  // 200 + the 5 sample downloads + the sample torrent.
  await expect(page.locator('.row')).toHaveCount(206)
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)
  expect(overflow).toBeLessThanOrEqual(0)
  // The 240-character name is cut with an ellipsis, not wrapped or overflowing.
  const long = page.locator('.row-name').filter({ hasText: 'aaaaaaaaaa' }).first()
  const box = await long.boundingBox()
  expect(box && box.x + box.width).toBeLessThanOrEqual(375)
  // Right-to-left and emoji names open fine; unknown and zero sizes read sensibly.
  await page.locator('.row-name').filter({ hasText: 'تقرير' }).first().click()
  await expect(page.locator('article.detail')).toBeVisible()
  await page.getByRole('button', { name: 'Back to downloads' }).click()
  await page.locator('.row-name').filter({ hasText: 'zero-bytes' }).first().click()
  await expect(page.locator('article.detail .facts')).toContainText('0\u00A0B')
})

test('oklch tokens convert to the sRGB values in DESIGN-SYSTEM.md (canvas fallback)', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  const out = await page.evaluate(async () => {
    // Served by Vite at runtime; typed via the real module.
    const path = '/src/lib/color.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/color')
    return {
      fuse: m.parseOklch('oklch(0.74 0.17 50)'),
      canvas: m.parseOklch('oklch(0.155 0.01 260)'),
      tide: m.parseOklch('oklch(0.78 0.12 215)'),
      withAlpha: m.parseOklch('oklch(1 0 0 / 0.08)'),
      percent: m.parseOklch('oklch(74% 0.17 50)'),
      junk: [m.parseOklch('rgb(1,2,3)'), m.parseOklch('oklch(a b c)'), m.parseOklch('')],
    }
  })
  const near = (got: number[] | null, hex: string) => {
    expect(got).not.toBeNull()
    const want = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16))
    got!.forEach((v, i) => expect(Math.abs(v - want[i]!)).toBeLessThanOrEqual(2))
  }
  near(out.fuse, '#fd8537')
  near(out.canvas, '#0a0c11')
  near(out.tide, '#43cae7')
  near(out.withAlpha, '#ffffff')
  expect(out.percent).toEqual(out.fuse)
  expect(out.junk).toEqual([null, null, null])
})

test('a paused job opened first still draws its ring in colour, not black', async ({ page }) => {
  await page.goto('/?drop=0')
  await page.getByText('dataset-shard-0042.tar.zst').click()
  await expect(page.locator('article.detail .speed-sub')).toHaveText('Paused')
  await page.waitForTimeout(400)
  const orange = await page.locator('[data-testid="fuse-core"] canvas').evaluate((el) => {
    const c = el as HTMLCanvasElement
    const d = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data
    let hits = 0
    for (let i = 0; i < d.length; i += 4) {
      const [r, g, b, a] = [d[i]!, d[i + 1]!, d[i + 2]!, d[i + 3]!]
      if (a > 200 && r > 150 && r - g > 60 && g > b) hits++
    }
    return hits
  })
  expect(orange).toBeGreaterThan(50) // the Fuse progress arc
})

test('on a Retina screen the canvas never widens the detail pane', async ({ browser }) => {
  for (const width of [1024, 1180, 1440]) {
    const ctx = await browser.newContext({ viewport: { width, height: 760 }, deviceScaleFactor: 2 })
    const page = await ctx.newPage()
    await page.goto('/?drop=0')
    await expect(page.getByTestId('fuse-core')).toBeVisible()
    await page.waitForTimeout(500)
    const extra = await page.locator('.pane-detail').evaluate((e) => e.scrollWidth - e.clientWidth)
    expect(extra, `at ${width}px`).toBeLessThanOrEqual(0)
    await ctx.close()
  }
})

test('a finished download keeps its network colours and shows who carried what', async ({
  page,
}) => {
  await page.goto('/?speed=50&drop=0')
  const detail = page.locator('article.detail')
  await expect(detail.locator('.speed-sub')).toHaveText('Done', { timeout: 15000 })
  const table = page.getByRole('table', { name: 'Networks in this download' })
  await expect(table.getByRole('columnheader', { name: 'Carried' })).toBeVisible()
  await expect(table).toContainText('Ethernet')
  await page.waitForTimeout(2200) // let the ignition sweep fade
  const lanes = await page.locator('[data-testid="fuse-core"] canvas').evaluate((el) => {
    const c = el as HTMLCanvasElement
    const d = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data
    let cool = 0
    for (let i = 0; i < d.length; i += 4) {
      const [r, g, b, a] = [d[i]!, d[i + 1]!, d[i + 2]!, d[i + 3]!]
      if (a > 200 && b > r + 30) cool++ // blue-ish lane ticks, not the orange of a plain ring
    }
    return cool
  })
  expect(lanes).toBeGreaterThan(50)
})

test('finished files can be opened or shown; a moved file explains itself', async ({ page }) => {
  await page.goto('/?missing=1')
  await page.getByText('Blender-5.1-macos-arm64.dmg').click()
  const detail = page.locator('article.detail')
  await expect(detail.getByRole('button', { name: 'Open' })).toBeVisible()
  const show = detail.getByRole('button', { name: /^Show in (Finder|Explorer|folder)$/ })
  await show.click()
  await expect(page.getByRole('alert').filter({ hasText: 'no longer at' })).toContainText(
    'moved, renamed or deleted',
  )
  // Unfinished downloads offer neither.
  await page.getByText('dataset-shard-0042.tar.zst').click()
  await expect(detail.getByRole('button', { name: 'Open' })).toHaveCount(0)
})

test('Choose… fills the folder from the native picker, and cancelling leaves it alone', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await dialog.getByRole('button', { name: 'Choose…' }).click()
  await expect(dialog.getByLabel('Save to')).toHaveValue('/Users/demo/Movies')
  await page.goto('/?empty=1&pick=cancel')
  await page.getByRole('button', { name: 'New download' }).first().click()
  await dialog.getByLabel('Save to').fill('/keep/me')
  await dialog.getByRole('button', { name: 'Choose…' }).click()
  await expect(dialog.getByLabel('Save to')).toHaveValue('/keep/me')
})

test('the dialog previews a link before downloading it', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  const link = dialog.getByLabel('Link')
  const help = dialog.locator('#nd-url-help')
  await link.fill('https://mirror.example.org/isos/fedora-43.iso')
  await expect(help).toContainText('fedora-43.iso, 700 MB. Splits across all your networks.')
  await link.fill('https://noranges.example.org/blob.bin')
  await expect(help).toContainText("won't split the file")
  await link.fill('https://unknown.example.org/stream.bin')
  await expect(help).toContainText('size unknown')
  await link.fill('https://example.org/missing.bin')
  await expect(help).toContainText("doesn't exist (404)")
  // A preview failure warns but doesn't block: the backend decides on Download.
  await expect(dialog.getByRole('button', { name: 'Download', exact: true })).toBeEnabled()
})

test('pasting or dropping a link anywhere opens the dialog with it', async ({ page }) => {
  await page.goto('/?empty=1')
  await expect(page.getByText('Nothing downloading yet')).toBeVisible() // app is listening
  await page.evaluate(() => {
    const data = new DataTransfer()
    data.setData('text/plain', 'https://example.org/pasted.zip')
    document.body.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true }))
  })
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await expect(dialog.getByLabel('Link')).toHaveValue('https://example.org/pasted.zip')
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await page.evaluate(() => {
    const data = new DataTransfer()
    data.setData('text/uri-list', '# a comment line\nhttps://example.org/dropped.iso\n')
    document.body.dispatchEvent(
      new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }),
    )
  })
  await expect(dialog.getByLabel('Link')).toHaveValue('https://example.org/dropped.iso')
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  // Dropping something that isn't a link does nothing.
  await page.evaluate(() => {
    const data = new DataTransfer()
    data.setData('text/plain', 'javascript:alert(1)')
    document.body.dispatchEvent(
      new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }),
    )
  })
  await expect(dialog).toBeHidden()
})

test('an expired link is fixed in place and the download continues', async ({ page }) => {
  await page.goto('/?drop=0')
  await page.getByText('nightly-build-2026-10-07.zip').click()
  const field = page.getByLabel('New link to the same file')
  const go = page.getByRole('button', { name: 'Continue' })
  // A bad link is refused right there, with the cursor still in the field.
  await field.fill('ftp://mirror.example.org/nightly.zip')
  await go.click()
  await expect(page.locator('.fix-link .field-error')).toContainText("ftp: links aren't supported")
  await expect(field).toHaveAttribute('aria-invalid', 'true')
  await field.fill('https://cdn2.example.org/nightly-build-2026-10-07.zip?sig=fresh')
  await go.click()
  await expect(page.locator('article.detail .speed')).toContainText('MB/s', { timeout: 5000 })
  await expect(page.getByLabel('New link to the same file')).toHaveCount(0)
})

test('a file that changed on the server can be started over', async ({ page }) => {
  await page.goto('/?drop=0')
  await page.getByText('mirror-snapshot-2026-10.tar').click()
  const detail = page.locator('article.detail')
  await expect(detail.getByRole('alert')).toContainText('changed during the download')
  // It can't resume or take a new link: only a fresh start.
  await expect(detail.getByRole('button', { name: 'Resume' })).toHaveCount(0)
  await expect(page.getByLabel('New link to the same file')).toHaveCount(0)
  await detail.getByRole('button', { name: 'Start over' }).click()
  await expect(detail.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  await expect(page.locator('.row-name', { hasText: 'mirror-snapshot' })).toHaveCount(1)
})

test('an overall speed limit is validated, saves itself and can be removed', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const field = page.getByLabel('Speed limit for all networks', { exact: true })
  for (const bad of ['abc', '-5', '1e9', '5 MB']) {
    await field.fill(bad)
    await field.press('Enter')
    await expect(page.getByText('Enter a number, like 5 or 2.5.')).toBeVisible()
    // Nothing invalid is saved.
    await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toHaveCount(0)
  }
  // A number too big to be a speed is refused by the backend with a clear reason.
  await field.fill('999999999')
  await field.press('Enter')
  await expect(
    page.getByRole('alert').filter({ hasText: 'too high to be a real speed' }),
  ).toBeVisible()
  await field.fill('2.5')
  await page.getByLabel('Speed limit for all networks unit').selectOption('MB')
  // It saves by itself once typing stops.
  await expect(
    page.getByRole('status').filter({ hasText: 'Running downloads follow it now' }),
  ).toBeVisible()
  await field.fill('')
  await field.press('Enter')
  await expect(page.getByRole('status').filter({ hasText: 'Limit removed' })).toBeVisible()
})

test("a network's limit saves on its own and survives moving between pages", async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Networks' }).click()
  const phone = page.getByLabel('iPhone USB speed limit', { exact: true })
  await phone.fill('512')
  await page.getByLabel('iPhone USB speed limit unit').selectOption('KB')
  await page.getByLabel('iPhone USB speed limit unit').press('Enter')
  await expect(page.getByRole('status').filter({ hasText: 'follow it now' })).toBeVisible()
  await page.getByRole('button', { name: 'Downloads' }).click()
  await page.getByRole('button', { name: 'Networks' }).click()
  await expect(page.getByLabel('iPhone USB speed limit', { exact: true })).toHaveValue('512')
  await expect(page.getByLabel('iPhone USB speed limit unit')).toHaveValue('KB')
  await expect(page.getByLabel('Wi-Fi speed limit', { exact: true })).toHaveValue('')
})

test('Copy diagnostics shows exactly what is copied, with nothing personal', async ({
  page,
  context,
  browserName,
}) => {
  if (browserName === 'chromium')
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).click()
  // From the keyboard: WebKit can move the long Settings page's scroll between a pointer
  // click being aimed and landing, so the click sometimes missed the button.
  await page.getByRole('button', { name: 'Copy diagnostics' }).focus()
  await page.keyboard.press('Enter')
  const report = page.getByLabel('Diagnostics report')
  await expect(report).toContainText('Recent downloads')
  await expect(page.getByRole('status').filter({ hasText: /Copied|Select the text/ })).toBeVisible()
  const text = await report.inputValue()
  expect(text).not.toMatch(/https?:\/\//)
  expect(text).not.toMatch(/\d+\.\d+\.\d+\.\d+/)
  expect(text).not.toContain('ubuntu-26.04')
})

test('an available update says its size and can be put off', async ({ page }) => {
  await page.goto('/?empty=1&update=1')
  const banner = page.locator('.update-banner')
  await expect(banner).toContainText('Fuselane 0.1.0-beta.8 is ready to download, 38.6')
  await banner.getByRole('button', { name: 'Later' }).click()
  await expect(banner).toHaveCount(0)
})

test('an update downloads with a progress bar, sizes and speed, then installs', async ({
  page,
}) => {
  await page.goto('/?empty=1&update=1')
  const banner = page.locator('.update-banner')
  await banner.getByRole('button', { name: 'Update and restart' }).click()
  const bar = banner.getByRole('progressbar', { name: 'Update download' })
  await expect(bar).toBeVisible()
  await expect(banner).toContainText('over 3 networks')
  await expect(banner.locator('.update-progress .num')).toContainText(
    /MB of 38\.6\u00a0MB, .+MB\/s/,
  )
  await expect(banner).toContainText('Installing 0.1.0-beta.8', { timeout: 10_000 })
})

test('an update download can be cancelled and offered again', async ({ page }) => {
  await page.goto('/?empty=1&update=slow')
  const banner = page.locator('.update-banner')
  await banner.getByRole('button', { name: 'Update and restart' }).click()
  await expect(banner.getByRole('progressbar')).toBeVisible()
  await banner.getByRole('button', { name: 'Cancel' }).click()
  await expect(banner.getByRole('button', { name: 'Update and restart' })).toBeVisible()
  await expect(banner.getByRole('progressbar')).toHaveCount(0)
})

test('a failed update says why and offers Try again', async ({ page }) => {
  await page.goto('/?empty=1&update=bad')
  const banner = page.locator('.update-banner')
  await banner.getByRole('button', { name: 'Update and restart' }).click()
  const failed = page.getByRole('alert').filter({ hasText: "Couldn't download the update" })
  await expect(failed).toBeVisible({ timeout: 10_000 })
  await expect(failed).toContainText('dropped at 22.0 MB')
  // Nothing was installed: Try again starts it over.
  await failed.getByRole('button', { name: 'Try again' }).click()
  await expect(banner.getByRole('progressbar')).toBeVisible()
})

test('update checks: quiet when offline at launch, clear when asked', async ({ page }) => {
  await page.goto('/?empty=1&update=offline')
  await page.waitForTimeout(800) // the launch check has run
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Check for updates' }).click()
  await expect(
    page.getByRole('alert').filter({ hasText: "Couldn't check for updates" }),
  ).toBeVisible()
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Check for updates' }).click()
  await expect(page.getByRole('status').filter({ hasText: "You're up to date." })).toBeVisible()
})

test('a network can be renamed and recoloured, and the name shows everywhere', async ({ page }) => {
  await page.goto('/?drop=0')
  await page.getByRole('button', { name: 'Networks' }).click()
  await page.getByRole('button', { name: 'Rename or recolour Wi-Fi' }).click()
  const name = page.getByLabel('Name', { exact: true })
  await name.fill('x'.repeat(41))
  await expect(page.getByText('Use up to 40 characters.')).toBeVisible()
  await expect(page.locator('.net-editor').getByRole('button', { name: 'Save' })).toBeDisabled()
  await name.fill('Home Wi-Fi')
  await page.getByRole('radio', { name: 'rose' }).check()
  await page.locator('.net-editor').getByRole('button', { name: 'Save' }).click()
  await expect(page.locator('.net-card .net-name').first()).toHaveText('Home Wi-Fi')
  // The running download's network table uses the new name too.
  await page.getByRole('button', { name: 'Downloads' }).click()
  await expect(page.getByRole('table', { name: 'Networks in this download' })).toContainText(
    'Home Wi-Fi',
    { timeout: 5000 },
  )
  // Reset brings the system name back.
  await page.getByRole('button', { name: 'Networks' }).click()
  await page.getByRole('button', { name: 'Rename or recolour Home Wi-Fi' }).click()
  await page.locator('.net-editor').getByRole('button', { name: 'Reset' }).click()
  await expect(page.locator('.net-card .net-name').first()).toHaveText('Wi-Fi')
})

test('slow mode switches on and off and its speed can be changed', async ({ page }) => {
  await page.goto('/?empty=1')
  const toggle = page.locator('.sidebar').getByRole('switch', { name: 'Slow mode' })
  await expect(toggle).toBeEnabled() // ready: the backend is connected
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.locator('.page').getByRole('switch', { name: 'Slow mode' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await page.getByLabel('Slow mode speed', { exact: true }).fill('300')
  await page.getByLabel('Slow mode speed unit').selectOption('KB')
  await page.getByLabel('Slow mode speed unit').press('Enter')
  await expect(page.locator('.sidebar .slow-toggle')).toContainText('300 KB/s max')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
})

test('keyboard: views, list movement, Space to pause and resume', async ({ page }) => {
  await page.goto('/?drop=0')
  await expect(
    page.getByRole('heading', { level: 1, name: 'ubuntu-26.04-desktop-amd64.iso' }),
  ).toBeVisible()
  await page.locator('body').click({ position: { x: 600, y: 890 } })
  await page.keyboard.press('Control+2')
  await expect(page.getByRole('heading', { level: 1, name: 'Networks' })).toBeVisible()
  await page.keyboard.press('Control+4')
  await expect(page.getByRole('heading', { level: 1, name: 'Send' })).toBeVisible()
  await page.keyboard.press('Control+3')
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible()
  await expect(page.getByText('Pause or resume the selected download')).toBeVisible()
  await page.keyboard.press('Control+1')
  // Down moves to the next download.
  await page.keyboard.press('ArrowDown')
  await expect(page.locator('article.detail h1')).not.toHaveText('ubuntu-26.04-desktop-amd64.iso')
  await page.keyboard.press('ArrowUp')
  await expect(page.locator('article.detail h1')).toHaveText('ubuntu-26.04-desktop-amd64.iso')
  // Space pauses, then resumes, the selected download.
  await page.keyboard.press(' ')
  await expect(page.locator('article.detail .speed-sub')).toHaveText('Paused')
  await page.keyboard.press(' ')
  await expect(page.locator('article.detail .speed')).toContainText('MB/s', { timeout: 5000 })
})

test('keyboard shortcuts never fire while typing in a field', async ({ page }) => {
  await page.goto('/?drop=0')
  await page.getByRole('button', { name: 'Settings' }).click()
  const field = page.getByLabel('Speed limit for all networks', { exact: true })
  await field.click()
  await page.keyboard.press(' ')
  await page.keyboard.press('ArrowDown')
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible()
  await page.getByRole('button', { name: 'Downloads' }).click()
  await expect(page.locator('article.detail .speed')).toContainText('MB/s', { timeout: 5000 })
})

test('after an update, a one-time message says it worked', async ({ page }) => {
  await page.goto('/?empty=1&updated=0.1.0-beta.1')
  const done = page.locator('.update-banner.updated')
  await expect(done).toContainText('Fuselane was updated to 0.0.0')
  await expect(done).toContainText('Your downloads and settings are kept.')
  await expect(done.getByRole('button', { name: "What's new" })).toBeVisible()
  await done.getByRole('button', { name: 'Close' }).click()
  await expect(done).toHaveCount(0)
  // A normal launch shows nothing.
  await page.goto('/?empty=1')
  await expect(page.locator('.update-banner.updated')).toHaveCount(0)
})

test('a monthly allowance can be set, is validated, and shows its usage', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Networks' }).click()
  const amount = page.getByLabel('iPhone USB monthly allowance')
  const row = page.locator('.allowance', { has: amount })
  await amount.fill('lots')
  await amount.press('Enter')
  await expect(row.getByText('Enter a number, like 5 or 2.5.')).toBeVisible()
  await expect(row.getByText('Saved')).toHaveCount(0)
  await amount.fill('8')
  await page.getByLabel('iPhone USB allowance unit').selectOption('GB')
  await page.getByLabel('iPhone USB reset day').selectOption('15')
  await page.getByLabel('iPhone USB reset day').press('Enter')
  const saved = page.locator('.allowance', { has: page.getByLabel('iPhone USB monthly allowance') })
  await expect(saved).toContainText('of 8.0 GB')
  await expect(page.getByLabel('iPhone USB reset day')).toHaveValue('15')
})

test('a reached allowance is explained, and a paused download points to the fix', async ({
  page,
}) => {
  await page.goto('/?allowance=reached&drop=0')
  await page.getByText('conference-talk-4k.mp4').click()
  await expect(page.getByRole('alert')).toContainText('every network reached its data allowance')
  await page.locator('article.detail').getByRole('button', { name: 'Open Networks' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Networks' })).toBeVisible()
  await expect(
    page.getByText(/Allowance reached: Fuselane won.t use this network until/),
  ).toBeVisible()
})

test('per-network lookups are off by default, explained, and can be turned on', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const toggle = page.getByRole('switch', { name: 'Look up servers through each network' })
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await expect(page.getByText(/Cloudflare and Google DNS/)).toBeVisible()
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
})

test('a network behind a sign-in page says so and offers the page', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/?portal=en0')
  await expect(page.locator('.sidebar-nets').getByText('Sign in needed')).toBeVisible()
  await page.getByRole('button', { name: 'Networks' }).first().click()
  const note = page.getByRole('note')
  await expect(note).toContainText('wants you to sign in')
  await expect(note.getByRole('button', { name: 'Open sign-in page' })).toBeVisible()
  // Only that network: the others read normally.
  await expect(page.locator('.net-cards').getByText('Sign in needed')).toHaveCount(1)
})

for (const [w, h] of [
  [1024, 700],
  [1280, 800],
  [1440, 900],
  [1920, 1080],
] as [number, number][]) {
  test(`nothing in the detail pane is cut off at ${w}x${h}`, async ({ page }) => {
    await page.setViewportSize({ width: w, height: h })
    await page.goto('/?freeze=4')
    await expect(page.getByTestId('fuse-core')).toBeVisible()
    const clipped = await page.evaluate(() => {
      const d = document.querySelector('.detail') as HTMLElement
      const pane = d.getBoundingClientRect()
      const out = [...d.querySelectorAll('.facts, .nets, .stream, .core')].filter(
        (el) => el.getBoundingClientRect().right > pane.right + 1,
      )
      return { overflow: d.scrollWidth - d.clientWidth, clipped: out.map((e) => e.className) }
    })
    expect(clipped).toEqual({ overflow: 0, clipped: [] })
  })
}

test('several links or a pattern become a batch, and repeats are skipped with a reason', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  const link = dialog.getByLabel('Link')
  await link.fill('https://example.com/a.iso\nhttps://example.com/b.iso\nhttps://example.com/c.iso')
  await expect(dialog.getByLabel('Links')).toBeVisible()
  await expect(dialog.locator('#nd-url-help')).toContainText('3 links')
  await expect(dialog.getByText('More options')).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Download 3' }).click()
  await expect(dialog).toBeHidden()
  for (const n of ['a.iso', 'b.iso', 'c.iso']) await expect(page.getByText(n).first()).toBeVisible()

  // A pattern, overlapping what is already there.
  const again = await openDialog(page)
  await again.getByLabel('Link').fill('https://example.com/[a-d].iso')
  await expect(again.locator('#nd-url-help')).toContainText('pattern')
  await again.getByRole('button', { name: 'Download all' }).click()
  await expect(again.getByRole('alert')).toContainText('3 links were not added')
  await expect(again.getByRole('alert')).toContainText('already added')
  await expect(page.getByText('d.iso').first()).toBeVisible()
})

test('a chosen name and SHA-256 are offered under More options and checked', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/file.bin')
  await dialog.getByText('More options').click()
  await dialog.getByLabel('SHA-256 to check').fill('abc')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog.locator('#nd-sha-help')).toContainText('64 hex digits')
  await expect(dialog.getByLabel('SHA-256 to check')).toHaveAttribute('aria-invalid', 'true')
  await dialog.getByLabel('SHA-256 to check').fill('a'.repeat(64))
  await dialog.getByLabel('Save as').fill('renamed.bin')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('heading', { level: 1, name: 'renamed.bin' })).toBeVisible()
})

test('adding a link twice asks first, and Download again adds it anyway', async ({ page }) => {
  await page.goto('/?empty=1')
  let dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/twice.iso')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByText('twice.iso').first()).toBeVisible()
  const before = await page.getByText('twice.iso').count()
  dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/twice.iso')
  await dialog.getByLabel('Link').press('Enter')
  await expect(dialog.getByRole('alert')).toContainText('You already added this link')
  await dialog.getByRole('button', { name: 'Download again' }).click()
  await expect(dialog).toBeHidden()
  await expect.poll(() => page.getByText('twice.iso').count()).toBeGreaterThan(before)
})

test('settings: downloads at once, schedule, when done, keep awake and sorting', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).click()
  const group = page.getByRole('group', { name: 'Downloads at once' })
  await expect(group.locator('output')).toHaveText('3')
  await group.getByRole('button', { name: 'One more' }).click()
  await expect(group.locator('output')).toHaveText('4')
  await expect(page.getByText('Up to 4 at once.')).toBeVisible()

  const schedule = page.getByRole('switch', { name: 'Download only on a schedule' })
  await expect(schedule).toHaveAttribute('aria-checked', 'false')
  await schedule.click()
  await expect(schedule).toHaveAttribute('aria-checked', 'true')
  await expect(page.getByLabel('From', { exact: true })).toHaveValue('01:00')
  await expect(page.getByText('Runs overnight')).toHaveCount(0)
  await page.getByLabel('From', { exact: true }).fill('23:00')
  await expect(page.getByText('Runs overnight: from 23:00 until 07:00')).toBeVisible()
  // No days chosen is refused with a reason.
  for (const d of ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'])
    await page.getByRole('button', { name: d }).click()
  await page
    .getByRole('form', { name: 'Download schedule' })
    .getByRole('button', { name: 'Save' })
    .click()
  await expect(page.getByText('Pick at least one day')).toBeVisible()

  const whenDone = page.getByRole('radiogroup', { name: 'When everything finishes' })
  await whenDone.getByRole('radio', { name: 'Sleep' }).click()
  // Changing another setting keeps the schedule edits that aren't saved yet.
  await expect(page.getByLabel('From', { exact: true })).toHaveValue('23:00')
  await expect(whenDone.getByRole('radio', { name: 'Sleep' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await expect(page.getByText('60 seconds and a notification')).toBeVisible()

  const awake = page.getByRole('switch', { name: 'Keep the computer awake' })
  await expect(awake).toHaveAttribute('aria-checked', 'true')
  await awake.click()
  await expect(awake).toHaveAttribute('aria-checked', 'false')
  const sort = page.getByRole('switch', { name: 'Sort into folders by type' })
  await sort.click()
  await expect(sort).toHaveAttribute('aria-checked', 'true')
})

test('the when-done countdown can be cancelled', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('Demo data')).toBeVisible()
  await page.evaluate(() =>
    (window as unknown as { __demoEvent: (e: unknown) => void }).__demoEvent({
      type: 'whenDone',
      action: 'shut-down',
      seconds: 60,
    }),
  )
  const banner = page.getByRole('alert').filter({ hasText: 'All downloads finished' })
  await expect(banner).toContainText('Your computer shuts down in')
  await banner.getByRole('button', { name: 'Cancel' }).click()
  await expect(banner).toBeHidden()
})

test('the list can be searched and filtered, with counts', async ({ page }) => {
  await page.goto('/')
  const search = page.getByRole('searchbox', { name: 'Search downloads' })
  const filters = page.getByRole('radiogroup', { name: 'Show' })
  await expect(filters.getByRole('radio', { name: /^All/ })).toHaveAttribute('aria-checked', 'true')
  await search.fill('ubuntu')
  await expect(page.locator('.row-name', { hasText: 'ubuntu' })).toBeVisible()
  await expect(page.locator('.row-name', { hasText: 'Blender' })).toHaveCount(0)
  await search.fill('no such file anywhere')
  await expect(page.getByText('Nothing matches "no such file anywhere".')).toBeVisible()
  await page.getByRole('button', { name: 'Show everything' }).click()
  await expect(search).toHaveValue('')
  await filters.getByRole('radio', { name: /^Finished/ }).click()
  await expect(page.locator('.row-name', { hasText: 'Blender' })).toBeVisible()
  await expect(page.locator('.row-name', { hasText: 'ubuntu' })).toHaveCount(0)
  await filters.getByRole('radio', { name: /^Failed/ }).click()
  await expect(page.locator('.row[data-status="failed"]').first()).toBeVisible()
})

test('speeds can show in Mbps as well as MB/s', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
  await page.getByRole('button', { name: 'Settings' }).click()
  const unit = page.getByRole('radiogroup', { name: 'Speed unit' })
  await unit.getByRole('radio', { name: 'Mbps' }).click()
  await page.getByRole('button', { name: 'Downloads' }).first().click()
  await expect(page.locator('.speed')).toContainText('Mbps', { timeout: 5000 })
  await page.reload()
  await expect(page.locator('.speed')).toContainText('Mbps', { timeout: 5000 })
})

test('start at login, keep running and copied links are switches, off at first', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).click()
  for (const name of [
    'Start at login',
    'Keep running when the window closes',
    'Catch copied download links',
  ]) {
    const s = page.getByRole('switch', { name })
    await expect(s).toHaveAttribute('aria-checked', 'false')
    await s.click()
    await expect(s).toHaveAttribute('aria-checked', 'true')
  }
})

test('the download list exports and imports from Settings', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Import…' }).click()
  await expect(page.getByText('Added 2 downloads.')).toBeVisible()
  await page.getByRole('button', { name: 'Import…' }).click()
  await expect(page.getByText('Added 0 downloads; skipped 2')).toBeVisible()
  await page.getByRole('button', { name: 'Export…' }).click()
  await expect(page.getByText('Saved 2 links.')).toBeVisible()
  await page.getByRole('button', { name: 'Downloads' }).first().click()
  await expect(page.getByText('one.iso')).toBeVisible()
  await expect(page.locator('.row', { hasText: 'two.zip' })).toContainText('Paused')
})

test('a copied download link opens the dialog with it', async ({ page }) => {
  await page.goto('/?empty=1')
  await expect(page.getByText('Nothing downloading yet')).toBeVisible()
  // The app's clipboard watcher hands links over like the OS does.
  await page.evaluate(() =>
    (window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(
      'https://example.com/copied.zip',
    ),
  )
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await expect(dialog.getByLabel('Link')).toHaveValue('https://example.com/copied.zip')
})

test('a download that failed for a passing reason says it will try again', async ({ page }) => {
  await page.goto('/?offline=1')
  await page.getByText('podcast-episode-212.mp3').click()
  await expect(page.getByText(/Fuselane tries again by itself in about 2 min/)).toBeVisible()
  await page.getByRole('button', { name: 'Try again' }).click()
  await expect(page.getByText(/tries again by itself/)).toHaveCount(0)
})

test('a finished file can go to the Trash from the remove dialog', async ({ page }) => {
  await page.goto('/')
  await page.getByText('Blender-5.1-macos-arm64.dmg').click()
  await page.locator('article.detail').getByRole('button', { name: 'Remove' }).click()
  await page
    .getByRole('dialog', { name: 'Remove this download?' })
    .getByRole('button', { name: /^Move file to (Trash|Recycle Bin)$/ })
    .click()
  await expect(page.getByText('Blender-5.1-macos-arm64.dmg')).toHaveCount(0)
})

test('Do this now gives one download every network, and the others carry on after', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('button', { name: /dataset-shard-0042/ })
    .first()
    .click()
  await page.getByRole('button', { name: 'Do this now' }).click()
  const every = page.getByRole('button', { name: 'Every network', exact: true })
  await expect(every).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: 'Do this now' })).toHaveCount(0)
  await expect(page.getByLabel('Has every network')).toBeVisible()
  // The one that was running waits, and says why.
  await page
    .getByRole('button', { name: /ubuntu-26\.04/ })
    .first()
    .click()
  await expect(page.getByRole('status').filter({ hasText: 'goes first' })).toBeVisible()
  await page
    .getByRole('button', { name: /dataset-shard-0042/ })
    .first()
    .click()
  await every.click()
  await page
    .getByRole('button', { name: /ubuntu-26\.04/ })
    .first()
    .click()
  await expect(page.locator('article.detail .speed')).toContainText('MB/s', { timeout: 5000 })
})

test('a download checked against a published checksum says Verified, and it can be turned off', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('button', { name: /Blender-5\.1/ })
    .first()
    .click()
  const badge = page.locator('article.detail .checksum-badge')
  await expect(badge).toContainText('Verified')
  await expect(badge).toContainText('against SHA256SUMS')
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Settings' })
    .click()
  const sw = page.getByRole('switch', { name: 'Check downloads against published checksums' })
  await expect(sw).toHaveAttribute('aria-checked', 'true')
  await sw.click()
  await expect(sw).toHaveAttribute('aria-checked', 'false')
})

test('a file already downloaded is pointed out before downloading it again', async ({ page }) => {
  await page.goto('/?drop=0')
  const dialog = await openDialog(page)
  await dialog
    .getByLabel('Link')
    .fill('https://downloads.example.org/files/Blender-5.1-macos-arm64.dmg')
  // Here it's in the same folder too, so it joins the "name is taken" choice.
  const note = dialog.getByRole('group', { name: /You already downloaded this/ })
  await expect(note).toContainText('412 MB')
  await expect(note.getByRole('button', { name: /^Show in|^Open folder/ })).toBeVisible()
  // Downloading again is still possible.
  await expect(dialog.getByRole('button', { name: 'Download', exact: true })).toBeEnabled()
  await dialog.getByLabel('Link').fill('https://downloads.example.org/files/something-else.iso')
  await expect(note).toHaveCount(0)
})

test('a finished download says what each network saved', async ({ page }) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('button', { name: /Blender-5\.1/ })
    .first()
    .click()
  const panel = page.getByRole('region', { name: 'What each network saved' })
  await expect(panel).toContainText('Finished in 30 s')
  await expect(panel.getByRole('listitem')).toHaveCount(3)
  await expect(panel.getByRole('listitem').filter({ hasText: 'Ethernet' })).toContainText('saved')
  // A download still running has nothing to report yet.
  await page
    .getByRole('button', { name: /ubuntu-26\.04/ })
    .first()
    .click()
  await expect(page.getByRole('region', { name: 'What each network saved' })).toHaveCount(0)
})

test('a network can wait for long downloads, with the minutes that count as long', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Networks' })
    .click()
  const phone = page.getByRole('radiogroup', { name: 'iPhone USB' })
  await expect(phone.getByRole('radio', { name: 'Always' })).toHaveAttribute('aria-checked', 'true')
  await expect(page.getByLabel('A long download takes more than')).toHaveCount(0)
  await phone.getByRole('radio', { name: 'Long downloads' }).click()
  await expect(phone.getByRole('radio', { name: 'Long downloads' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  const minutes = page.getByLabel('A long download takes more than')
  await expect(minutes).toHaveValue('5')
  await minutes.fill('12')
  await page.locator('.long-minutes').getByRole('button', { name: 'Save' }).click()
  await expect(page.locator('.long-minutes').getByRole('button', { name: 'Save' })).toBeDisabled()
  // Its name and colour are kept when it's renamed later, and the choice survives.
  await page
    .getByRole('radiogroup', { name: 'Ethernet' })
    .getByRole('radio', { name: 'Never' })
    .click()
  await expect(phone.getByRole('radio', { name: 'Long downloads' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
})

test('what happens on low battery is a setting', async ({ page }) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Settings' })
    .click()
  const choice = page.getByRole('radiogroup', { name: 'On low battery' })
  await expect(choice.getByRole('radio', { name: 'Keep going' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await choice.getByRole('radio', { name: 'Leave out the phone' }).click()
  await expect(page.getByText(/a phone tethered by cable is left out/)).toBeVisible()
  await choice.getByRole('radio', { name: 'Pause' }).click()
  await expect(page.getByText(/downloads pause and carry on when you plug in/)).toBeVisible()
})

test('a download can be given a time to be ready by, and says how it is doing', async ({
  page,
}) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('button', { name: /dataset-shard-0042/ })
    .first()
    .click()
  const field = page.getByLabel('Ready by')
  const form = page.locator('.ready-by')
  await expect(field).not.toHaveValue('') // a time is offered
  await field.fill('23:59')
  await form.getByRole('button', { name: 'Set' }).click()
  await expect(field).toHaveValue('23:59')
  await expect(form.locator('.field-help')).toContainText(/Ready by .*(On track|At risk)/)
  await expect(
    page.locator('.row', { hasText: 'dataset-shard-0042' }).locator('.row-ready'),
  ).toContainText('Ready by')
  await form.getByRole('button', { name: 'Clear' }).click()
  await expect(form.getByRole('button', { name: 'Clear' })).toHaveCount(0)
  await expect(
    page.locator('.row', { hasText: 'dataset-shard-0042' }).locator('.row-ready'),
  ).toHaveCount(0)
})

test('links added together become one group row, paused and resumed as a whole', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog
    .getByLabel('Link')
    .fill(
      'https://media.example.org/s1/ep1.mkv\nhttps://media.example.org/s1/ep2.mkv\nhttps://media.example.org/s1/ep3.mkv',
    )
  await expect(
    dialog.getByRole('checkbox', { name: /Keep them together as a group/ }),
  ).toBeChecked()
  await dialog.getByLabel('Group name').fill('Season 1')
  await dialog.getByRole('button', { name: 'Download 3' }).click()
  await expect(dialog).toBeHidden()
  const head = page.locator('.group-row .row-main')
  await expect(head).toContainText('Season 1')
  await expect(head).toContainText('0 of 3 done')
  // It opens on its first download, which is selected.
  await expect(head).toHaveAttribute('aria-expanded', 'true')
  await expect(page.getByRole('list', { name: 'Season 1' }).getByRole('listitem')).toHaveCount(3)
  await page.getByRole('button', { name: 'Pause all in Season 1' }).click()
  await expect(page.getByRole('button', { name: 'Resume all in Season 1' })).toBeVisible()
  await page.getByRole('button', { name: 'Resume all in Season 1' }).click()
  await expect(page.getByRole('button', { name: 'Pause all in Season 1' })).toBeVisible()
  // Renamed, then (below) collapsed to a single row.
  await page.getByRole('button', { name: 'Rename' }).click()
  await page.getByRole('textbox', { name: 'Group name' }).fill('Season one')
  await page.locator('.group-tools').getByRole('button', { name: 'Save' }).click()
  await expect(head).toContainText('Season one')
  await head.click()
  await expect(head).toHaveAttribute('aria-expanded', 'false')
  await expect(page.getByRole('list', { name: 'Season one' })).toHaveCount(0)
  // Ungrouped, its downloads are single rows again.
  await head.click()
  await page.getByRole('button', { name: 'Ungroup' }).click()
  await expect(head).toHaveCount(0)
  await expect(page.getByText('ep2.mkv')).toBeVisible()
})

test('the files a page links to can be picked by type and added as a group', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://releases.example.org/26.04/')
  await dialog.getByRole('button', { name: 'Find files on this page' }).click()
  // The dialog now shows the page's files under its own title.
  const picker = page.getByRole('dialog', { name: 'Files on the page' })
  await expect(picker).toBeVisible()
  const files = picker.getByRole('list', { name: 'Files on the page' })
  await expect(files.getByRole('listitem')).toHaveCount(7)
  await expect(picker.getByRole('button', { name: 'Download', exact: true })).toBeDisabled()
  await picker
    .getByRole('group', { name: 'Pick by type' })
    .getByRole('button', { name: /^Disk images/ })
    .click()
  await expect(files.getByRole('checkbox', { checked: true })).toHaveCount(2)
  await files.getByRole('checkbox', { name: /release-notes\.pdf/ }).check()
  await picker.getByRole('button', { name: 'Download 3' }).click()
  await expect(picker).toBeHidden()
  await expect(page.getByRole('button', { name: /Ubuntu 26\.04 downloads/ }).first()).toContainText(
    '0 of 3 done',
  )
})

test('a paused download can continue on another computer with Fuselane', async ({ page }) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('button', { name: /dataset-shard-0042/ })
    .first()
    .click()
  await page.getByRole('button', { name: 'Continue elsewhere' }).click()
  const dialog = page.getByRole('dialog', { name: 'Continue on another computer' })
  await expect(dialog).toBeVisible()
  // Only other Fuselane computers are offered (a phone with LocalSend isn't).
  await expect(dialog.getByRole('button', { name: "Maya's MacBook Air" })).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Pixel 9' })).toHaveCount(0)
  await dialog.getByRole('button', { name: "Maya's MacBook Air" }).click()
  await expect(dialog.getByRole('status')).toContainText("Sent to Maya's MacBook Air")
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(dialog).toBeHidden()
  // A running download offers no hand-off.
  await page
    .getByRole('button', { name: /ubuntu-26\.04/ })
    .first()
    .click()
  await expect(page.getByRole('button', { name: 'Continue elsewhere' })).toHaveCount(0)
})

test('after a crash Fuselane offers once to report it, and problems can be reported from Settings', async ({
  page,
}) => {
  await page.goto('/?drop=0&crash=1')
  const banner = page.locator('.crash-banner')
  await expect(banner).toContainText('closed unexpectedly')
  await banner.getByRole('button', { name: 'Report it' }).click()
  await expect(banner).toHaveCount(0)
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Settings' })
    .click()
  await expect(page.getByRole('button', { name: 'Report a problem' })).toBeVisible()
})

test('a video page offers its qualities and downloads the chosen one', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://www.youtube.com/watch?v=aqz-KE-bpKQ')
  await dialog.getByRole('button', { name: 'Get the video from this page' }).click()
  const pick = page.getByRole('dialog', { name: 'Get the video' })
  await expect(pick).toContainText('Big Buck Bunny')
  await expect(pick.getByRole('radio', { name: /2160p/ })).toBeChecked()
  await pick.getByRole('radio', { name: /1080p/ }).check()
  await pick.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(pick).toBeHidden()
  await expect(
    page.getByRole('heading', { level: 1, name: 'Big Buck Bunny (1080p).mp4' }),
  ).toBeVisible()
})

test('the extension hands a video page over and the qualities open by themselves', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  await expect(page.getByText('Nothing downloading yet')).toBeVisible()
  await page.evaluate(() =>
    (window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(
      'fuselane-video:https://www.youtube.com/watch?v=aqz-KE-bpKQ',
    ),
  )
  const pick = page.getByRole('dialog', { name: 'Get the video' })
  await expect(pick).toContainText('Big Buck Bunny')
  await expect(pick.getByRole('radio', { name: /1080p/ })).toBeVisible()
})

test('without yt-dlp a video page says what to install', async ({ page }) => {
  await page.goto('/?empty=1&ytdlp=0')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://www.youtube.com/watch?v=aqz-KE-bpKQ')
  await dialog.getByRole('button', { name: 'Get the video from this page' }).click()
  await expect(dialog.getByRole('alert')).toContainText('brew install yt-dlp')
})

test('a network can be used only at certain hours', async ({ page }) => {
  await page.goto('/?drop=0')
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Networks' })
    .click()
  const row = page.getByRole('article', { name: 'iPhone USB' })
  const from = row.getByLabel('iPhone USB from')
  await expect(from).toBeDisabled()
  await row.getByRole('checkbox', { name: 'Only from' }).check()
  await expect(from).toBeEnabled()
  await expect(from).toHaveValue('23:00')
  await expect(row.getByLabel('iPhone USB until')).toHaveValue('06:00')
  await row.getByRole('checkbox', { name: 'Only from' }).uncheck()
  await expect(from).toBeDisabled()
  // Daily data expires at midnight: one click for the last two hours.
  await row
    .getByRole('button', { name: 'iPhone USB: only before midnight, 22:00 to 00:00' })
    .click()
  await expect(row.getByRole('checkbox', { name: 'Only from' })).toBeChecked()
  await expect(from).toHaveValue('22:00')
  await expect(row.getByLabel('iPhone USB until')).toHaveValue('00:00')
})

test('Download later adds it paused, ready to start', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/later.iso')
  await dialog.getByRole('button', { name: 'Download later' }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('heading', { level: 1, name: 'later.iso' })).toBeVisible()
  await expect(page.locator('.facts')).toContainText('Paused')
  await page.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(page.locator('.facts')).toContainText('Time left')
  // A magnet goes through its file list first, so it has no "later" button.
  const again = await openDialog(page)
  await again.getByLabel('Link').fill('magnet:?xt=urn:btih:' + 'a'.repeat(40))
  await expect(again.getByRole('button', { name: 'Download later' })).toHaveCount(0)
})

test('a download can have its own speed limit, changed while it runs', async ({ page }) => {
  await page.goto('/')
  await expect(
    page.getByRole('heading', { level: 1, name: 'ubuntu-26.04-desktop-amd64.iso' }),
  ).toBeVisible()
  const field = page.getByLabel('Speed limit for this download', { exact: true })
  const set = page.getByRole('button', { name: 'Set limit' })
  await expect(set).toBeDisabled()
  await field.fill('fast')
  await expect(page.getByText('Enter a number, like 5 or 2.5.')).toBeVisible()
  await expect(set).toBeDisabled()
  await field.fill('2')
  await set.click()
  await expect(set).toBeDisabled()
  // The live speed settles at or under the limit.
  await expect
    .poll(
      async () => {
        const t = (await page.locator('.speed').first().textContent()) ?? ''
        return Number(t.replace(/[^\d.]/g, ''))
      },
      { timeout: 8000 },
    )
    .toBeLessThanOrEqual(2.2)
  await field.fill('')
  await page.getByRole('button', { name: 'Remove limit' }).click()
  await expect(field).toHaveValue('')
})

test.describe('Fuse Send', () => {
  const LINK = 'https://arshpunisher.github.io/fuselane/s#v1.AbCdEfGhIjKlMnOpQrStUv'

  async function openSend(page: Page) {
    await page
      .getByRole('navigation', { name: 'Main' })
      .getByRole('button', { name: 'Send' })
      .click()
    await expect(page.getByRole('heading', { level: 1, name: 'Send' })).toBeVisible()
    // Links live on the Link tab; Nearby is the default.
    await page
      .getByRole('radiogroup', { name: 'How to send' })
      .getByRole('radio', { name: 'Link' })
      .click()
  }

  test('choosing a file prepares it and gives a link to copy', async ({
    page,
    context,
    browserName,
  }) => {
    await page.goto('/?empty=1&speed=4')
    await openSend(page)
    await page.getByRole('button', { name: /Choose a file to send/ }).click()
    const list = page.getByRole('list', { name: "Files you're sending" })
    await expect(list).toContainText('Holiday video.mov')
    const link = list.getByLabel('Link for Holiday video.mov')
    await expect(link).toHaveValue(/^https:\/\/fuselane\.app\/s#v1\./)
    await expect(list).toContainText('Waiting for the receiver')
    if (browserName === 'chromium') {
      await context.grantPermissions(['clipboard-read', 'clipboard-write'])
      await list.getByRole('button', { name: 'Copy link' }).click()
      await expect(list.getByRole('button', { name: 'Copied' })).toBeVisible()
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
        await link.inputValue(),
      )
    }
    await list.getByRole('button', { name: 'Stop sending Holiday video.mov' }).click()
    await expect(list).toHaveCount(0)
  })

  test('a share can stop by itself after one full copy is sent', async ({ page }) => {
    await page.goto('/?empty=1&sends=1&speed=10')
    await openSend(page)
    const list = page.getByRole('list', { name: "Files you're sending" })
    await list.getByLabel('Stop sharing after one full copy is sent').check()
    await expect(list).toContainText('Sent in full. Sharing stopped', { timeout: 10_000 })
    await expect(list.getByLabel('Link for Wedding photos.zip')).toHaveCount(0)
    await list.getByRole('button', { name: 'Stop sending Wedding photos.zip' }).click()
    await expect(list).toHaveCount(0)
  })

  test('cancelling the file picker does nothing', async ({ page }) => {
    await page.goto('/?empty=1&pick=cancel')
    await openSend(page)
    await page.getByRole('button', { name: /Choose a file to send/ }).click()
    await expect(page.getByRole('list', { name: "Files you're sending" })).toHaveCount(0)
  })

  test('a link is received, checked, and can be shown or cleared', async ({ page }) => {
    await page.goto('/?empty=1&speed=6')
    await openSend(page)
    const field = page.getByLabel('Link someone sent you')
    const receive = page.getByRole('button', { name: 'Receive', exact: true })
    await receive.click()
    await expect(page.getByRole('alert')).toContainText('Paste the link someone sent you')
    await expect(field).toBeFocused()
    await field.fill('https://example.com/not-a-share')
    await receive.click()
    await expect(page.getByRole('alert')).toContainText("isn't a whole Fuse Send link")
    await expect(field).toHaveAttribute('aria-invalid', 'true')
    await field.fill(LINK)
    await receive.click()
    await expect(field).toHaveValue('')
    const list = page.getByRole('list', { name: "Files you're receiving" })
    await expect(list).toContainText('Arrived and checked', { timeout: 10_000 })
    await expect(list).toContainText('Holiday video.mov')
    await list.getByRole('button', { name: 'Show' }).click()
    await list.getByRole('button', { name: 'Remove Holiday video.mov from the list' }).click()
    await expect(list).toHaveCount(0)
  })

  test('an offline sender is explained', async ({ page }) => {
    await page.goto('/?empty=1&speed=6')
    await openSend(page)
    await page.getByLabel('Link someone sent you').fill(`${LINK}offline`)
    await page.getByRole('button', { name: 'Receive', exact: true }).click()
    const list = page.getByRole('list', { name: "Files you're receiving" })
    await expect(list).toContainText("Didn't arrive", { timeout: 10_000 })
    await expect(list).toContainText('keep it open until the file arrives')
  })

  test('pasting a Send link anywhere opens it on the Send page', async ({ page }) => {
    await page.goto('/?empty=1')
    await expect(page.getByRole('button', { name: 'New download' }).first()).toBeEnabled()
    await page.evaluate((text) => {
      const data = new DataTransfer()
      data.setData('text/plain', text)
      document.body.dispatchEvent(
        new ClipboardEvent('paste', { clipboardData: data, bubbles: true }),
      )
    }, LINK)
    await expect(page.getByRole('heading', { level: 1, name: 'Send' })).toBeVisible()
    await expect(page.getByLabel('Link someone sent you')).toHaveValue(LINK)
    await expect(page.getByLabel('Link someone sent you')).toBeFocused()
    await expect(page.getByRole('dialog', { name: 'New download' })).toBeHidden()
  })

  test('a Send link in the download dialog goes to the Send page instead', async ({ page }) => {
    await page.goto('/?empty=1')
    const dialog = await openDialog(page)
    await dialog.getByLabel('Link').fill(LINK)
    await dialog.getByRole('button', { name: 'Download', exact: true }).click()
    await expect(dialog).toBeHidden()
    await expect(page.getByLabel('Link someone sent you')).toHaveValue(LINK)
  })

  test('the share page handing a link to the app opens it on the Send page', async ({ page }) => {
    await page.goto('/?empty=1')
    await expect(page.getByText('Nothing downloading yet')).toBeVisible()
    const link = `fuselane://send/${LINK.split('#')[1]}`
    await page.evaluate((t) => {
      ;(window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(t)
    }, link)
    await expect(page.getByLabel('Link someone sent you')).toHaveValue(link)
    await expect(page.getByRole('dialog', { name: 'New download' })).toBeHidden()
  })

  test('the Send page fits a phone', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 })
    await page.goto('/?sends=1')
    await page
      .getByRole('navigation', { name: 'Main' })
      .getByRole('button', { name: 'Send' })
      .click()
    // Nearby fits too: the radar shrinks, nothing sideways.
    await expect(page.locator('.radar-node').first()).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
    await page.getByRole('radio', { name: 'Link' }).click()
    await expect(page.getByText('Wedding photos.zip')).toBeVisible()
    await expect(page.getByText('Band demo.wav')).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
  })
})

test('the link box is one line: no scrollbar, long links cut in the middle, magnets become a card', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  const link = dialog.getByLabel('Link')
  const long =
    'https://releases.ubuntu.com/26.04/daily-live/current/builds/2026-10-09/ubuntu-26.04-desktop-amd64.iso'
  await link.fill(long)
  // One row, nothing to scroll vertically, however long the link.
  const box = await link.evaluate((el) => ({
    rows: (el as HTMLTextAreaElement).rows,
    scrolls: el.scrollHeight > el.clientHeight + 1,
    overflowY: getComputedStyle(el).overflowY,
  }))
  expect(box).toEqual({ rows: 1, scrolls: false, overflowY: 'hidden' })
  // Away from the field, the file name stays visible at the end.
  await dialog.getByLabel('Save to').focus()
  await expect(dialog.locator('.link-cut .tail')).toHaveText('/ubuntu-26.04-desktop-amd64.iso')
  await link.focus()
  await expect(dialog.locator('.link-cut')).toHaveCount(0)
  // A magnet becomes a card named from its dn=, with Change to edit it.
  await link.fill(
    'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=Sprite+Fright+%282021%29+4K&tr=udp%3A%2F%2Ftracker.example%3A1337',
  )
  await expect(dialog.locator('.link-card-name')).toHaveText('Sprite Fright (2021) 4K')
  await expect(dialog.locator('.link-card-meta')).toContainText('01234567')
  await dialog.getByRole('button', { name: 'Change the link' }).click()
  await expect(link).toBeFocused()
  await link.fill('https://example.com/a.iso')
  // Shift+Enter starts a list; a list shows its lines.
  await link.press('Shift+Enter')
  await link.pressSequentially('https://example.com/b.iso')
  await expect(dialog.getByLabel('Links')).toHaveAttribute('rows', '3')
})

test('a download can start at a set time, and starts by itself when it comes', async ({ page }) => {
  // 23:58 local time, so "00:00" is two minutes ahead (tomorrow).
  const now = new Date()
  now.setHours(23, 58, 0, 0)
  await page.clock.install({ time: now })
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/night.iso')
  await dialog.getByText('More options').click()
  await dialog.getByRole('radio', { name: 'At a time' }).click()
  await dialog.getByLabel('Start time').fill('00:00')
  await expect(dialog.getByText(/^tomorrow at /)).toBeVisible()
  // "Download later" makes no sense next to a start time.
  await expect(dialog.getByRole('button', { name: 'Download later' })).toHaveCount(0)
  await dialog.getByRole('button', { name: /^Download at / }).click()
  await expect(dialog).toBeHidden()
  const row = page.locator('.row', { hasText: 'night.iso' })
  await expect(row.locator('.row-state')).toHaveText(/^Starts tomorrow at /)
  // Two minutes later it starts by itself.
  await page.clock.fastForward('02:05')
  await expect(row.locator('.row-state')).toHaveText('Downloading')
})

test('starting a scheduled download by hand drops its start time', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://example.com/later.iso')
  await dialog.getByText('More options').click()
  await dialog.getByRole('radio', { name: 'At a time' }).click()
  await dialog.getByRole('button', { name: /^Download at / }).click()
  const row = page.locator('.row', { hasText: 'later.iso' })
  await expect(row.locator('.row-state')).toHaveText(/^Starts /)
  await expect(
    page.locator('article.detail').getByRole('button', { name: 'Start now' }),
  ).toBeVisible()
  await row.getByRole('button', { name: 'Resume later.iso' }).click()
  await expect(row.locator('.row-state')).toHaveText('Downloading')
})

test('a taken name is asked about in New download: keep both, replace, or skip', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  const dialog = await openDialog(page)
  // The demo list already has this file.
  await dialog.getByLabel('Link').fill('https://mirror.example.net/ubuntu-26.04-desktop-amd64.iso')
  const ask = dialog.getByRole('group', { name: /is already in this folder/ })
  await expect(ask).toBeVisible()
  await expect(ask.getByRole('radio', { name: 'Keep both' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await ask.getByRole('radio', { name: 'Replace' }).click()
  await expect(ask.getByRole('radio', { name: 'Replace' })).toHaveAttribute('aria-checked', 'true')
  // A name that isn't taken asks nothing.
  await dialog.getByLabel('Link').fill('https://example.com/brand-new.iso')
  await expect(ask).toHaveCount(0)
  await dialog.getByLabel('Link').fill('https://mirror.example.net/ubuntu-26.04-desktop-amd64.iso')
  await ask.getByRole('button', { name: "Don't download" }).click()
  await expect(dialog).toBeHidden()
})

test('settings: when a download finishes, and if the name is taken', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const after = page.getByRole('radiogroup', { name: 'When a download finishes' })
  await after.getByRole('radio', { name: 'Unpack it' }).click()
  await expect(after.getByRole('radio', { name: 'Unpack it' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await expect(page.getByText(/Zip and tar archives unpack into a folder/)).toBeVisible()
  const taken = page.getByRole('radiogroup', { name: 'If the name is taken' })
  await taken.getByRole('radio', { name: 'Ask' }).focus()
  await page.keyboard.press('ArrowRight')
  await expect(taken.getByRole('radio', { name: 'Keep both' })).toHaveAttribute(
    'aria-checked',
    'true',
  )
  await expect(taken.getByRole('radio', { name: 'Keep both' })).toBeFocused()
  // With Keep both chosen, New download no longer asks.
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await dialog.getByLabel('Link').fill('https://example.com/x.iso')
  await expect(dialog.getByRole('group', { name: /is already in this folder/ })).toHaveCount(0)
})

test('the list can show one type of file', async ({ page }) => {
  await page.goto('/?freeze=3')
  const type = page.getByRole('combobox', { name: 'Type' })
  await expect(type.locator('option')).toContainText(['All types'])
  await type.selectOption('disk-images')
  // Only disk images (iso, dmg) remain; the torrent and the zip are hidden.
  await expect(
    page.locator('.row-name', { hasText: 'ubuntu-26.04-desktop-amd64.iso' }),
  ).toBeVisible()
  await expect(page.locator('.row-name', { hasText: 'Blender-5.1-macos-arm64.dmg' })).toBeVisible()
  await expect(page.locator('.row-name', { hasText: 'nightly-build-2026-10-07.zip' })).toHaveCount(
    0,
  )
  await expect(page.locator('.row-name', { hasText: 'Sprite Fright' })).toHaveCount(0)
  await type.selectOption('torrents')
  await expect(page.locator('.row-name', { hasText: 'Sprite Fright' })).toBeVisible()
  await expect(page.locator('.row-name', { hasText: 'ubuntu' })).toHaveCount(0)
})

test('mirrors can be added to a download and are listed on it', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  await dialog.getByLabel('Link').fill('https://releases.example.org/os.iso')
  await dialog.getByText('More options').click()
  await dialog.getByRole('button', { name: 'Add a mirror' }).click()
  await dialog.getByLabel('Mirror 1', { exact: true }).fill('ftp://old.example.net/os.iso')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  // A bad mirror is explained under the field, and the dialog stays.
  await expect(dialog.getByText(/A mirror link isn't usable/)).toBeVisible()
  await dialog.getByLabel('Mirror 1', { exact: true }).fill('https://mirror.example.net/os.iso')
  // Editing clears the error; wait for that re-render before the next click (WebKit on CI).
  await expect(dialog.getByText(/A mirror link isn't usable/)).toBeHidden()
  await dialog.getByRole('button', { name: 'Add a mirror' }).click()
  await expect(dialog.getByLabel('Mirror 2', { exact: true })).toBeVisible()
  await dialog.getByLabel('Mirror 2', { exact: true }).fill('https://other.example.com/pub/os.iso')
  await dialog.getByRole('button', { name: 'Remove mirror 2' }).click()
  await expect(dialog.getByLabel('Mirror 2', { exact: true })).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.locator('article.detail')).toContainText(
    'Also from a mirror: mirror.example.net',
  )
})

test.describe('Nearby', () => {
  async function openNearby(page: Page, query = '') {
    await page.goto(`/?freeze=3${query}`)
    await page
      .getByRole('navigation', { name: 'Main' })
      .getByRole('button', { name: 'Send' })
      .click()
    await expect(page.getByRole('radio', { name: 'Nearby' })).toHaveAttribute(
      'aria-checked',
      'true',
    )
  }

  test('devices sit around this computer, and sending shows a beam, progress, then Done', async ({
    page,
  }) => {
    await openNearby(page)
    const radar = page.locator('.radar')
    await expect(radar.locator('.radar-me')).toContainText("Arsh's MacBook Pro")
    await expect(radar.locator('.radar-node')).toHaveCount(4)
    await expect(radar.getByRole('button', { name: 'Send files to Pixel 9' })).toContainText(
      'via LocalSend',
    )
    await radar.getByRole('button', { name: "Send files to Maya's MacBook Air" }).click()
    await expect(radar.locator('.beam')).toHaveCount(1)
    const row = page.getByRole('complementary', { name: 'Activity' }).locator('.activity').first()
    await expect(row).toContainText("To Maya's MacBook Air")
    await expect(row).toHaveAttribute('data-state', 'done', { timeout: 10_000 })
    await expect(row.locator('.done-check')).toBeVisible()
    await expect(radar.locator('.beam')).toHaveCount(0)
  })

  test('files dropped on a device lift it, then go to it', async ({ page }) => {
    await openNearby(page)
    const ravi = page.getByRole('button', { name: "Send files to Ravi's ThinkPad" })
    await ravi.scrollIntoViewIfNeeded()
    await page.waitForTimeout(700) // devices finish springing in
    const box = (await ravi.boundingBox())!
    const at = { x: box.x + box.width / 2, y: box.y + 30 }
    const drop = (type: string) =>
      page.evaluate(
        ([type, x, y]) =>
          (
            window as unknown as {
              __demoFileDrop: (e: { type: string; paths: string[]; x: number; y: number }) => void
            }
          ).__demoFileDrop({ type: type as string, paths: ['/Users/demo/a.mov'], x: +x!, y: +y! }),
        [type, at.x, at.y] as const,
      )
    await drop('over')
    await expect(ravi).toHaveAttribute('data-over', 'true')
    await drop('drop')
    await expect(ravi).not.toHaveAttribute('data-over', 'true')
    await expect(page.locator('.activity').first()).toContainText("To Ravi's ThinkPad")
  })

  test('a decline is reported', async ({ page }) => {
    await openNearby(page, '&nearby=decline')
    await page.getByRole('button', { name: "Send files to Ravi's ThinkPad" }).click()
    await expect(page.locator('.activity').first()).toContainText('Waiting')
    await expect(page.locator('.activity').first()).toContainText('They declined')
  })

  test('who can see this computer: trusted only by default, everyone for 10 minutes', async ({
    page,
  }) => {
    await openNearby(page)
    const vis = page.getByRole('radiogroup', { name: 'Who can see this computer' })
    await expect(vis.getByRole('radio', { name: 'Trusted only' })).toHaveAttribute(
      'aria-checked',
      'true',
    )
    await vis.getByRole('radio', { name: 'Everyone, 10 min' }).click()
    await expect(page.locator('.visibility')).toContainText(/for (10:00|9:5\d) more/)
    await expect(page.locator('.vis-ic .ring-fill')).toBeVisible()
    await vis.getByRole('radio', { name: 'Trusted only' }).click()
    await expect(page.locator('.visibility')).toContainText('Only devices you trust')
  })

  test('an incoming file asks, can be trusted, and a device can be forgotten', async ({ page }) => {
    await openNearby(page, '&nearby=request')
    const ask = page.getByRole('dialog', { name: /wants to send you a file/ })
    await expect(ask).toBeVisible()
    await expect(ask).not.toContainText('Check words')
    await expect(ask.getByRole('button', { name: 'Decline' })).toBeFocused()
    await ask.getByRole('checkbox', { name: /Trust Ravi's ThinkPad/ }).check()
    await ask.getByRole('button', { name: 'Accept' }).click()
    await expect(ask).toBeHidden()
    await expect(page.locator('.activity').first()).toContainText('From Ravi')
    const trusted = page.locator('.trusted')
    await expect(trusted).toContainText("Ravi's ThinkPad")
    await trusted.getByRole('button', { name: "Forget Ravi's ThinkPad" }).click()
    await expect(trusted).not.toContainText("Ravi's ThinkPad")
  })

  test('the receiver can cancel a file that is coming in', async ({ page }) => {
    await openNearby(page, '&nearby=request')
    await page.getByRole('button', { name: 'Accept' }).click()
    const row = page.locator('.activity[data-dir="in"]')
    await expect(row.locator('.activity-bar')).toBeVisible()
    await row.getByRole('button', { name: 'Cancel Holiday video.mov' }).click()
    await expect(row).toHaveAttribute('data-state', 'cancelled')
    await expect(row.getByRole('button', { name: /^Cancel/ })).toHaveCount(0)
  })

  test('text goes to another computer, and incoming text shows what it says', async ({ page }) => {
    await openNearby(page, '&nearby=text')
    const ask = page.getByRole('dialog', { name: /wants to send you text/ })
    await expect(ask).toContainText('ssh studio@192.168.1.9')
    await ask.getByRole('button', { name: 'Accept' }).click()
    const got = page.locator('.activity[data-dir="in"]').first()
    await expect(got).toContainText('ssh studio@192.168.1.9')
    await expect(got.getByRole('button', { name: /Copy text from STUDIO-PC/ })).toBeVisible()
    const card = page.getByRole('region', { name: 'Send text' })
    await card.getByLabel('Text to send').fill('hello from the laptop')
    await card.getByLabel('Send to').selectOption({ label: 'STUDIO-PC' })
    await card.getByRole('button', { name: 'Send', exact: true }).click()
    await expect(page.locator('.activity[data-dir="out"]').first()).toContainText(
      'hello from the laptop',
    )
    await expect(card.getByRole('button', { name: 'Send what I copied' })).toBeVisible()
  })

  test('a folder can be kept in sync with a trusted computer', async ({ page }) => {
    await openNearby(page)
    const card = page.getByRole('region', { name: 'Folders kept in sync' })
    await card.getByLabel('Keep in sync with').selectOption({ label: 'STUDIO-PC' })
    await card.getByRole('button', { name: 'Add a folder' }).click()
    await expect(card.getByRole('listitem')).toContainText('Movies → STUDIO-PC')
    await expect(card.getByRole('listitem')).toContainText('Up to date, 128 files', {
      timeout: 5000,
    })
    await card.getByRole('button', { name: 'Stop syncing Movies' }).click()
    await expect(card.getByRole('listitem')).toHaveCount(0)
  })

  test('a phone without the app gets a code, offered files, and Stop', async ({ page }) => {
    await openNearby(page)
    await page.getByRole('button', { name: 'Show a code to scan' }).click()
    await expect(
      page.getByRole('img', { name: "Code to scan with the phone's camera" }),
    ).toBeVisible()
    await expect(page.locator('.phone-text')).toContainText('http://')
    await page.getByRole('button', { name: 'Offer files to the phone' }).click()
    const offers = page.getByRole('list', { name: 'Offered to the phone' })
    await expect(offers).toContainText('Holiday video.mov')
    await offers.getByRole('button', { name: 'Stop offering Holiday video.mov' }).click()
    await expect(offers).toHaveCount(0)
    // Text for the phone: typed, or what was copied; then taken away.
    await page.getByRole('textbox', { name: 'Text for the phone' }).fill('wifi: hunter2')
    await page.getByRole('button', { name: 'Offer text' }).click()
    await expect(page.locator('.phone-note')).toContainText('wifi: hunter2')
    await page.getByRole('button', { name: "Take the text away from the phone's page" }).click()
    await page.getByRole('button', { name: 'Offer what I copied' }).click()
    await expect(page.locator('.phone-note')).toContainText('fuselane.app/faq')
    await page.getByRole('button', { name: 'Stop', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Show a code to scan' })).toBeVisible()
  })

  test('nobody on the network says what to do', async ({ page }) => {
    await openNearby(page, '&nearby=empty')
    await expect(page.locator('.radar-empty')).toContainText('Open Fuselane or LocalSend')
    await expect(page.locator('.radar')).toHaveAttribute('data-searching', 'true')
  })
})

test('remote control for aria2 apps: on, address, secret, network and port', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  const toggle = page.getByRole('switch', { name: 'Remote control for aria2 apps' })
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await expect(page.locator('.remote-facts')).toContainText('http://127.0.0.1:6800/jsonrpc')
  // The secret stays hidden until asked for; a new one replaces it.
  await expect(page.locator('.remote-facts')).not.toContainText('9f2c41d0')
  await page.getByRole('button', { name: 'Show the secret' }).click()
  await expect(page.locator('.remote-facts')).toContainText('9f2c41d07be35a68c1e4f0d2a7b96e13')
  await page.getByRole('button', { name: 'New secret' }).click()
  await expect(page.getByRole('status').filter({ hasText: 'New secret made.' })).toBeVisible()
  await expect(page.locator('.remote-facts')).not.toContainText('9f2c41d07be35a68')
  // Devices on the network get their own address.
  await page.getByRole('checkbox', { name: /Allow phones and computers/ }).check()
  await expect(page.locator('.remote-facts')).toContainText('http://192.168.1.24:6800/jsonrpc')
  // A code for the phone's remote page, only when asked for.
  await expect(page.getByRole('img', { name: 'Code to scan with your phone' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Show a code for your phone' }).click()
  await expect(page.getByRole('img', { name: 'Code to scan with your phone' })).toBeVisible()
  await page.getByRole('button', { name: 'Hide the code' }).click()
  // A port below 1024 is refused with the reason, a good one is saved.
  const port = page.getByRole('spinbutton', { name: 'Port' })
  await port.fill('80')
  await page.getByRole('button', { name: 'Save' }).last().click()
  await expect(page.locator('#remote-port-err')).toContainText('1024 to 65535')
  await port.fill('6801')
  await page.getByRole('button', { name: 'Save' }).last().click()
  await expect(page.locator('.remote-facts')).toContainText('http://127.0.0.1:6801/jsonrpc')
  await toggle.click()
  await expect(page.locator('.remote-facts')).toHaveCount(0)
})

test('feeds: follow one, filter words, check, open a torrent item, stop following', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Feeds', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Feeds' })
  const list = dialog.getByRole('list', { name: 'Feeds you follow' })
  await expect(list).toContainText('Tech & Talk')
  await expect(list).toContainText('12 downloaded')
  await expect(dialog.getByRole('list', { name: 'Latest in Tech & Talk' })).toContainText(
    'Skipped by your words',
  )
  // A web page instead of a feed: said plainly, with what to do.
  await dialog.getByLabel('Feed address').fill('https://example.com/not-a-feed')
  await dialog.getByRole('button', { name: 'Follow', exact: true }).click()
  await expect(dialog.locator('#feed-url-err')).toContainText("it's not RSS or Atom")
  await dialog.getByLabel('Feed address').fill('https://builds.example/atom')
  await dialog.getByLabel('Only titles with').first().fill('linux x64')
  await dialog.getByRole('button', { name: 'Follow', exact: true }).click()
  await expect(dialog.getByRole('status')).toContainText('The latest file is downloading')
  await expect(list).toContainText('Nightly builds')
  await expect(list).toContainText('only with “linux x64”')
  // Filters can change later.
  const first = list.locator('.feed').first()
  await first.getByRole('button', { name: 'Filters' }).click()
  await first.getByLabel('Skip titles with').fill('trailer teaser')
  await first.getByRole('button', { name: 'Save' }).click()
  await expect(list.locator('.feed').first()).toContainText('skipping “trailer teaser”')
  await list.getByRole('button', { name: 'Check Tech & Talk now' }).click()
  await expect(list.locator('.feed').first()).toContainText('Checked just now')
  await list.getByRole('button', { name: 'Stop following Nightly builds' }).click()
  await expect(list.locator('.feed')).toHaveCount(1)
  // A torrent in a feed waits; Open hands it to New download.
  await first.getByRole('button', { name: 'Open' }).click()
  await expect(page.getByRole('dialog', { name: 'New download' })).toBeVisible()
  await expect(dialog).toBeHidden()
})

test('a Metalink link adds the files it lists, as a group', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await dialog.getByLabel('Link').fill('https://mirrors.example/fedora/fedora-43.meta4')
  await expect(dialog.locator('#nd-url-help')).toContainText(
    'A Metalink: Fuselane downloads the files it lists',
  )
  await expect(dialog.getByRole('button', { name: 'Find files on this page' })).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('list', { name: '2 files from mirrors.example' })).toContainText(
    'fedora-43-x86_64.iso',
  )
})

test('watch folder: pick a folder, see what was taken, turn off', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  const toggle = page.getByRole('switch', { name: 'Add files from a folder' })
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
  await expect(page.locator('.watch-folder')).toContainText('/Users/demo/Movies')
  const taken = page.getByRole('list', { name: 'Files taken from the folder' })
  await expect(taken).toContainText('Torrent started')
  await expect(taken.getByRole('img', { name: 'Not added' })).toHaveCount(1)
  await toggle.click()
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
  await expect(page.locator('.watch')).toHaveCount(0)
})

test("the extension's feed button opens Feeds with the address ready", async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Feeds', exact: true })).toBeVisible()
  await page.evaluate(() =>
    (window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(
      'fuselane-feed:https://blog.example/feed.xml',
    ),
  )
  const dialog = page.getByRole('dialog', { name: 'Feeds' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByLabel('Feed address')).toHaveValue('https://blog.example/feed.xml')
})

test('welcome: networks, an optional check with a verdict, then tips', async ({ page }) => {
  await page.goto('/?welcome=1&fast=1')
  const welcome = page.getByRole('dialog', { name: 'Welcome to Fuselane' })
  await expect(welcome).toBeVisible()
  const nets = welcome.getByRole('list', { name: 'Networks found' })
  await expect(nets).toContainText('Wi-Fi')
  await expect(nets).toContainText('Ethernet')
  await expect(welcome.getByRole('button', { name: 'Next' })).toBeFocused()
  await welcome.getByText('Phone or second network not showing up?').click()
  await expect(welcome.getByText(/USB cable/).first()).toBeVisible()
  await welcome.getByRole('button', { name: 'Next' }).click()
  const step2 = page.getByRole('dialog', { name: 'How fast are they together?' })
  await expect(step2).toContainText('runs each network flat out for a few seconds')
  await step2.getByRole('button', { name: 'Run the check' }).click()
  await expect(step2.getByRole('status')).toContainText(/times .* alone/, { timeout: 15000 })
  await expect(step2.getByRole('list', { name: 'Check results' })).toContainText('Mbps')
  await step2.getByRole('button', { name: 'Next' }).click()
  const step3 = page.getByRole('dialog', { name: 'Ready when you are' })
  await expect(step3).toContainText('Paste a link anywhere')
  await step3.getByRole('button', { name: 'Start downloading' }).click()
  await expect(step3).toBeHidden()
})

test('welcome: Skip closes it; Settings shows it again; not shown otherwise', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Feeds', exact: true })).toBeVisible()
  await expect(page.getByRole('dialog', { name: 'Welcome to Fuselane' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Settings' }).first().click()
  await page.getByRole('button', { name: 'Show it again' }).click()
  const welcome = page.getByRole('dialog', { name: 'Welcome to Fuselane' })
  await expect(welcome).toBeVisible()
  await welcome.getByRole('button', { name: 'Skip' }).click()
  await expect(welcome).toBeHidden()
})

test('feeds: torrents start by themselves only when the feed allows it', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Feeds', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Feeds' })
  const list = dialog.getByRole('list', { name: 'Feeds you follow' })
  const talk = list.locator('.feed').filter({ hasText: 'Tech & Talk' })
  // Off unless turned on: the setting sits in the feed's Filters.
  await expect(talk).not.toContainText('Torrents start by themselves')
  await talk.getByRole('button', { name: 'Filters' }).click()
  const box = talk.getByRole('checkbox', { name: 'Start torrents by themselves' })
  await expect(box).not.toBeChecked()
  await box.check()
  await talk.getByRole('button', { name: 'Save' }).click()
  await expect(talk).toContainText('Torrents start by themselves.')
  // A new torrent in the feed starts, so there's nothing to open by hand.
  await talk.getByRole('button', { name: 'Check Tech & Talk now' }).click()
  const newest = dialog.getByRole('list', { name: 'Latest in Tech & Talk' }).locator('li').first()
  await expect(newest).toContainText('Episode 44 (torrent)')
  await expect(newest).toContainText('Torrent started')
  await expect(newest.getByRole('button', { name: 'Open' })).toHaveCount(0)
  await expect(talk).toContainText('13 downloaded')

  // The add form has it too, off until ticked.
  const addBox = dialog
    .locator('form.feed-add')
    .getByRole('checkbox', { name: 'Start torrents by themselves' })
  await expect(addBox).not.toBeChecked()
  await dialog.getByLabel('Feed address').fill('https://shows.example/rss')
  await addBox.check()
  await dialog.getByRole('button', { name: 'Follow', exact: true }).click()
  const shows = list.locator('.feed').filter({ hasText: 'Nightly builds' })
  await expect(shows).toContainText('Torrents start by themselves.')
  // Turned off again, its torrents wait for you.
  await shows.getByRole('button', { name: 'Filters' }).click()
  const showsBox = shows.getByRole('checkbox', { name: 'Start torrents by themselves' })
  await expect(showsBox).toBeChecked()
  await showsBox.uncheck()
  await shows.getByRole('button', { name: 'Save' }).click()
  await expect(shows).not.toContainText('Torrents start by themselves')
  await shows.getByRole('button', { name: 'Check Nightly builds now' }).click()
  const waiting = dialog
    .getByRole('list', { name: 'Latest in Nightly builds' })
    .locator('li')
    .first()
  await expect(waiting).toContainText('Torrent')
  await expect(waiting.getByRole('button', { name: 'Open' })).toBeVisible()
})

test.describe('Proxy per network', () => {
  async function openProxy(page: Page, net: string, query = '') {
    await page.goto(`/?drop=0${query}`)
    await page.getByRole('button', { name: 'Networks' }).first().click()
    await page.getByRole('button', { name: `Set up a proxy for ${net}` }).click()
    return page.getByRole('form', { name: `Proxy for ${net}` })
  }

  test('a proxy is validated, saved, checked, and its password never comes back', async ({
    page,
  }) => {
    const form = await openProxy(page, 'Wi-Fi')
    await expect(form.getByLabel('Address')).toBeFocused()
    // Empty fields are explained next to them, and the first one gets the focus.
    await form.getByRole('button', { name: 'Save' }).click()
    await expect(form.getByText("Enter the proxy's name or address.")).toBeVisible()
    await expect(form.getByText('Use a port from 1 to 65535, like 8080 or 1080.')).toBeVisible()
    await expect(form.getByLabel('Address')).toHaveAttribute('aria-invalid', 'true')
    await form.getByLabel('Address').fill('proxy.office.lan')
    await form.getByRole('button', { name: 'Save' }).click()
    await expect(form.getByLabel('Port')).toBeFocused()
    // A link instead of a name gets the service's plain answer and what to do.
    await form.getByLabel('Address').fill('http://proxy.office.lan:8080')
    await form.getByLabel('Port').fill('8080')
    await form.getByRole('button', { name: 'Save' }).click()
    await expect(form.getByRole('alert')).toContainText(
      "Enter just the proxy's name or address. Leave out http://",
    )
    await form.getByLabel('Address').fill('proxy.office.lan')
    await form.getByRole('radio', { name: 'SOCKS5' }).click()
    await form.getByLabel('Username').fill('ann')
    await form.getByLabel(/^Password/).fill('s3cret-pass')
    await form.getByRole('button', { name: 'Save' }).click()
    const row = page.locator('.proxy-row', { hasText: 'Wi-Fi' })
    await expect(row).toContainText('SOCKS5 proxy.office.lan:8080, as ann')
    // Saving checks it straight away.
    await expect(row.locator('.proxy-status')).toContainText(
      'Works: Wi-Fi reaches the internet through this proxy.',
    )
    // Editing shows that a password is saved, never the password itself.
    await row.getByRole('button', { name: 'Edit the proxy for Wi-Fi' }).click()
    const edit = page.getByRole('form', { name: 'Proxy for Wi-Fi' })
    await expect(edit.getByLabel(/^Password/)).toHaveValue('')
    await expect(edit.getByLabel(/^Password/)).toHaveAttribute('placeholder', 'Saved')
    await expect(
      edit.getByText("A password is saved in your system's keychain and never shown."),
    ).toBeVisible()
    expect(await page.content()).not.toContain('s3cret-pass')
    // Escape cancels and gives the focus back.
    await edit.getByLabel('Port').press('Escape')
    await expect(edit).toHaveCount(0)
    await expect(row.getByRole('button', { name: 'Edit the proxy for Wi-Fi' })).toBeFocused()
    // Torrents are told apart.
    await expect(page.locator('.net-notes')).toContainText("Torrents don't use these proxies")
    // Removing it goes back to direct.
    await row.getByRole('button', { name: 'Edit the proxy for Wi-Fi' }).click()
    await page
      .getByRole('form', { name: 'Proxy for Wi-Fi' })
      .getByRole('button', { name: 'Remove proxy' })
      .click()
    await expect(row).toContainText('Direct, no proxy')
  })

  test("without a system keychain the password is saved in Fuselane's settings, and says so", async ({
    page,
  }) => {
    const form = await openProxy(page, 'Wi-Fi', '&nokeychain=1')
    await form.getByLabel('Address').fill('proxy.office.lan')
    await form.getByLabel('Port').fill('3128')
    await form.getByLabel('Username').fill('ann')
    await form.getByLabel(/^Password/).fill('s3cret-pass')
    await form.getByRole('button', { name: 'Save' }).click()
    const row = page.locator('.proxy-row', { hasText: 'Wi-Fi' })
    await row.getByRole('button', { name: 'Edit the proxy for Wi-Fi' }).click()
    const edit = page.getByRole('form', { name: 'Proxy for Wi-Fi' })
    await expect(edit.getByLabel(/^Password/)).toHaveAttribute('placeholder', 'Saved')
    await expect(
      edit.getByText(
        "A password is saved in Fuselane's settings (no system keychain available) and never shown.",
      ),
    ).toBeVisible()
    expect(await page.content()).not.toContain('s3cret-pass')
  })

  test('a proxy that turns down the login says so, with what to do', async ({ page }) => {
    const form = await openProxy(page, 'iPhone USB')
    await form.getByLabel('Address').fill('10.0.0.2')
    await form.getByLabel('Port').fill('3128')
    await form.getByLabel('Username').fill('ann')
    await form.getByLabel(/^Password/).fill('wrong')
    await form.getByRole('button', { name: 'Save' }).click()
    const row = page.locator('.proxy-row', { hasText: 'iPhone USB' })
    await expect(row.locator('.proxy-status .field-error')).toHaveText(
      "iPhone USB's proxy at 10.0.0.2:3128 turned down the username and password. Check them in Networks, under Proxy.",
    )
    // Checking again after fixing the password works.
    await row.getByRole('button', { name: 'Edit the proxy for iPhone USB' }).click()
    const edit = page.getByRole('form', { name: 'Proxy for iPhone USB' })
    await edit.getByLabel(/^Password/).fill('right')
    await edit.getByRole('button', { name: 'Save' }).click()
    await expect(row.locator('.proxy-status')).toContainText('Works: iPhone USB reaches')
  })

  test('renaming a network keeps its proxy', async ({ page }) => {
    const form = await openProxy(page, 'Ethernet')
    await form.getByLabel('Address').fill('proxy.lan')
    await form.getByLabel('Port').fill('3128')
    await form.getByRole('button', { name: 'Save' }).click()
    await page.getByRole('button', { name: 'Rename or recolour Ethernet' }).click()
    await page.getByLabel('Name', { exact: true }).fill('Office cable')
    await page.locator('.net-editor').getByRole('button', { name: 'Save' }).click()
    const row = page.locator('.proxy-row', { hasText: 'Office cable' })
    await expect(row).toContainText('HTTP proxy.lan:3128')
  })
})

test('a download says when a network is throttled, comes back, or its proxy refuses', async ({
  page,
}) => {
  await page.goto('/?drop=0&throttle=1&proxytrouble=1')
  const notes = page.getByRole('list', { name: 'Network notes' })
  await expect(notes).toContainText(
    'iPhone USB slowed to 8.0 KB/s (throttled?), so the other networks carry the rest. Fuselane tries it again every few minutes.',
  )
  await expect(notes).toContainText(
    "Wi-Fi's proxy at proxy.office.lan:3128 turned down the username and password.",
  )
  // The phone rests: no speed of its own while the others carry the rest.
  await expect(page.getByRole('table', { name: 'Networks in this download' })).toContainText(
    'iPhone USB',
  )
  await page.goto('/?drop=0&throttle=back')
  await expect(page.getByRole('list', { name: 'Network notes' })).toContainText(
    "iPhone USB is fast again (1.2 MB/s), so it's helping again.",
  )
  // Without news, no notes at all.
  await page.goto('/?drop=0')
  await expect(page.getByRole('list', { name: 'Network notes' })).toHaveCount(0)
})
