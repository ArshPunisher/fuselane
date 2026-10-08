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
  [1920, 1080, 'wide'],
] as const) {
  test(`layout at ${w}x${h} is ${layout} with no sideways scroll`, async ({ page }) => {
    await page.setViewportSize({ width: w, height: h })
    await page.goto('/')
    await expect(page.locator('.app')).toHaveAttribute('data-layout', layout)
    await page.getByText('ubuntu-26.04-desktop-amd64.iso').first().click()
    await expect(page.getByTestId('fuse-core')).toBeVisible()
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)
    expect(overflow).toBeLessThanOrEqual(0)
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
