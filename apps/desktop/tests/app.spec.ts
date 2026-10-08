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
  const submit = dialog.getByRole('button', { name: 'Download' })
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
  await dialog.getByRole('button', { name: 'Download' }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('heading', { level: 1, name: 'linux image.iso' })).toBeVisible()
  await expect(page.locator('.speed')).toContainText('MB/s', { timeout: 5000 })
})

test('hostile file names render as text, never markup', async ({ page }) => {
  await page.goto('/?empty=1')
  const dialog = await openDialog(page)
  const evil = '<img src=x onerror="window.pwned=1">‮gpj.exe'
  await dialog.getByLabel('Link').fill(`https://example.com/${encodeURIComponent(evil)}`)
  await dialog.getByRole('button', { name: 'Download' }).click()
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
  await expect(page.locator('.row')).toHaveCount(204)
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
  await expect(dialog.getByRole('button', { name: 'Download' })).toBeEnabled()
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
