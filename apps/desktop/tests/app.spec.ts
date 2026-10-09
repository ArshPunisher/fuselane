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
  // First click only arms it.
  await expect(
    page.getByRole('heading', { level: 1, name: 'ubuntu-26.04-desktop-amd64.iso' }),
  ).toBeVisible()
  await detail.getByRole('button', { name: 'Delete partial file' }).click()
  await expect(page.locator('.row-name', { hasText: 'ubuntu-26.04' })).toHaveCount(0)
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
  await page.getByRole('radio', { name: 'System' }).click()
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.*/)
})

test('networks page lists usable networks and explains skipped ones', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Networks' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Networks' })).toBeVisible()
  await expect(page.locator('.page .netlist').first()).toContainText('iPhone USB')
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

test('an overall speed limit is validated, saved and can be removed', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const field = page.getByLabel('Speed limit for all networks', { exact: true })
  const save = page.getByRole('form', { name: 'Speed limit' }).getByRole('button', { name: 'Save' })
  await expect(save).toBeDisabled() // nothing changed yet
  for (const bad of ['abc', '-5', '1e9', '5 MB']) {
    await field.fill(bad)
    await expect(page.getByText('Enter a number, like 5 or 2.5.')).toBeVisible()
    await expect(save).toBeDisabled()
  }
  // A number too big to be a speed is refused by the backend with a clear reason.
  await field.fill('999999999')
  await save.click()
  await expect(
    page.getByRole('alert').filter({ hasText: 'too high to be a real speed' }),
  ).toBeVisible()
  await field.fill('2.5')
  await page.getByLabel('Speed limit for all networks unit').selectOption('MB')
  await save.click()
  await expect(
    page.getByRole('status').filter({ hasText: 'Running downloads follow it now' }),
  ).toBeVisible()
  await expect(save).toBeDisabled()
  await field.fill('')
  await save.click()
  await expect(page.getByRole('status').filter({ hasText: 'Limit removed' })).toBeVisible()
})

test('per-network limits save together and survive moving between pages', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'Networks' }).click()
  const phone = page.getByLabel('iPhone USB speed limit', { exact: true })
  await phone.fill('512')
  await page.getByLabel('iPhone USB speed limit unit').selectOption('KB')
  await page.getByRole('button', { name: 'Save limits' }).click()
  await expect(page.getByRole('status').filter({ hasText: 'follow these now' })).toBeVisible()
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
  await page.getByRole('button', { name: 'Copy diagnostics' }).click()
  const report = page.getByLabel('Diagnostics report')
  await expect(report).toContainText('Recent downloads')
  await expect(page.getByRole('status').filter({ hasText: /Copied|Select the text/ })).toBeVisible()
  const text = await report.inputValue()
  expect(text).not.toMatch(/https?:\/\//)
  expect(text).not.toMatch(/\d+\.\d+\.\d+\.\d+/)
  expect(text).not.toContain('ubuntu-26.04')
})

test('an available update is offered once and can be put off', async ({ page }) => {
  await page.goto('/?empty=1&update=1')
  const banner = page.locator('.update-banner')
  await expect(banner).toContainText('Fuselane 0.1.0-beta.2 is available')
  await banner.getByRole('button', { name: 'Later' }).click()
  await expect(banner).toHaveCount(0)
})

test('a bad update signature is refused with a plain message', async ({ page }) => {
  await page.goto('/?empty=1&update=bad')
  const banner = page.locator('.update-banner')
  await banner.getByRole('button', { name: 'Update and restart' }).click()
  await expect(page.getByRole('alert').filter({ hasText: "signature doesn't match" })).toBeVisible()
  // Nothing was installed: the offer is still there to retry later.
  await expect(banner.getByRole('button', { name: 'Update and restart' })).toBeEnabled()
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
  await expect(page.locator('.page .netlist .net-name').first()).toHaveText('Home Wi-Fi')
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
  await expect(page.locator('.page .netlist .net-name').first()).toHaveText('Wi-Fi')
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
  await page.getByRole('form', { name: 'Slow mode' }).getByRole('button', { name: 'Save' }).click()
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
  await expect(row.getByText('Enter a number, like 5 or 2.5.')).toBeVisible()
  await expect(row.getByRole('button', { name: 'Save' })).toBeDisabled()
  await amount.fill('8')
  await page.getByLabel('iPhone USB allowance unit').selectOption('GB')
  await page.getByLabel('iPhone USB reset day').selectOption('15')
  await row.getByRole('button', { name: 'Save' }).click()
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
  await expect(page.getByRole('main').getByText('Sign in needed')).toHaveCount(1)
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
  await expect(page.getByLabel('From')).toHaveValue('01:00')
  await expect(page.getByText('Runs overnight')).toHaveCount(0)
  await page.getByLabel('From').fill('23:00')
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
  await expect(page.getByLabel('From')).toHaveValue('23:00')
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
    await expect(link).toHaveValue(/^https:\/\/arshpunisher\.github\.io\/fuselane\/s#v1\./)
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
    await expect(page.getByText('Wedding photos.zip')).toBeVisible()
    await expect(page.getByText('Band demo.wav')).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
  })
})
