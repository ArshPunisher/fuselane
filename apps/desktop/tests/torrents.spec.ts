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

test('a torrent shows each network credited with verified bytes', async ({ page }) => {
  await page.goto('/?freeze=3')
  await expect(page.getByRole('heading', { name: /Torrents/ })).toBeVisible()
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  await expect(
    page.getByRole('heading', { level: 1, name: 'Sprite Fright (2021) 4K' }),
  ).toBeVisible()
  const table = page.getByRole('table', { name: 'Networks in this torrent' })
  await expect(table).toContainText('Ethernet')
  await expect(table).toContainText('Peers')
  // Shares add up to 100% (credit sums to the verified total).
  const shares = await table.locator('tbody tr td:nth-child(3)').allTextContents()
  expect(shares.reduce((a, s) => a + parseInt(s, 10), 0)).toBeGreaterThanOrEqual(99)
  await expect(page.locator('.torrent-head .speed')).toContainText('MB/s')
  await expect(page.getByText('2 of 5', { exact: true })).toBeVisible()
})

test('opening a .torrent lets you choose files before anything downloads', async ({ page }) => {
  await page.goto('/?torrents=0')
  const dialog = await openDialog(page)
  await dialog.getByRole('button', { name: 'Open .torrent…' }).click()
  const pick = page.getByRole('dialog', { name: 'Choose files' })
  await expect(pick).toBeVisible()
  await expect(pick.getByText('5 of 5')).toBeVisible()
  // Nothing chosen: Download is disabled.
  await pick.getByRole('checkbox', { name: 'All files' }).uncheck()
  await expect(pick.getByRole('button', { name: 'Download', exact: true })).toBeDisabled()
  await pick.getByRole('checkbox', { name: /Sprite Fright 1080p\.mkv/ }).check()
  await expect(pick.getByText(/1 of 5, 912/)).toBeVisible()
  // Back returns to the link form; going forward again keeps the picker working.
  await pick.getByRole('button', { name: 'Back' }).click()
  await expect(page.getByRole('dialog', { name: 'New download' })).toBeVisible()
  await page.getByRole('button', { name: 'Open .torrent…' }).click()
  const again = page.getByRole('dialog', { name: 'Choose files' })
  await again.getByRole('checkbox', { name: 'All files' }).uncheck()
  await again.getByRole('checkbox', { name: /English\.srt/ }).check()
  await again.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(page.getByRole('dialog')).toBeHidden()
  await expect(
    page.getByRole('heading', { level: 1, name: 'Sprite Fright (2021) 4K' }),
  ).toBeVisible()
  await expect(page.getByText('1 of 5', { exact: true })).toBeVisible()
})

test('magnet links are checked, then their files are found', async ({ page }) => {
  await page.goto('/?torrents=0')
  const dialog = await openDialog(page)
  const link = dialog.getByLabel('Link')
  await link.fill('magnet:?dn=no-hash')
  await expect(dialog.getByRole('button', { name: 'Next' })).toBeVisible()
  await dialog.getByRole('button', { name: 'Next' }).click()
  await expect(dialog.getByText(/isn't a magnet link/)).toBeVisible()
  await expect(link).toHaveAttribute('aria-invalid', 'true')
  await link.fill('magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=debian')
  await dialog.getByRole('button', { name: 'Next' }).click()
  await expect(dialog.getByRole('button', { name: 'Finding files…' })).toBeDisabled()
  const pick = page.getByRole('dialog', { name: 'Choose files' })
  await expect(pick).toBeVisible()
  await expect(pick.getByText('debian-13.1.0-amd64-DVD-1.iso').first()).toBeVisible()
})

test('a slow magnet lookup can be cancelled', async ({ page }) => {
  await page.goto('/?torrents=0&magnet=slow')
  const dialog = await openDialog(page)
  await dialog
    .getByLabel('Link')
    .fill('magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567')
  await dialog.getByRole('button', { name: 'Next' }).click()
  await expect(dialog.getByText(/This can take a minute/)).toBeVisible()
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(dialog.getByRole('button', { name: 'Next' })).toBeEnabled()
})

test('pasting a magnet anywhere opens the dialog with it', async ({ page }) => {
  await page.goto('/?torrents=0')
  await expect(page.getByText('Demo data')).toBeVisible()
  const magnet = 'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567'
  await page.evaluate((m) => {
    const e = new ClipboardEvent('paste', { clipboardData: new DataTransfer() })
    e.clipboardData?.setData('text', m)
    window.dispatchEvent(e)
  }, magnet)
  await expect(page.getByRole('dialog', { name: 'New download' }).getByLabel('Link')).toHaveValue(
    magnet,
  )
})

test('files can be changed mid-download and the torrent removed keeping files', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  const files = page.getByRole('region', { name: 'Files' })
  await files.getByRole('checkbox', { name: /poster\.jpg/ }).check()
  await files.getByRole('button', { name: 'Save choice' }).click()
  await expect(page.getByText('3 of 5', { exact: true })).toBeVisible()
  await expect(files.getByRole('button', { name: 'Save choice' })).toBeHidden()
  // A mixed "All files" selects everything first; unchecking then clears all, which can't be saved.
  const all = files.getByRole('checkbox', { name: /^All files/ })
  await all.check()
  await expect(files.getByText(/5 of 5/)).toBeVisible()
  await all.uncheck()
  await expect(files.getByRole('button', { name: 'Save choice' })).toBeDisabled()
  await files.getByRole('button', { name: 'Undo' }).click()

  await page.getByRole('button', { name: 'Remove' }).click()
  await page.getByRole('button', { name: 'Keep files' }).click()
  await expect(page.getByRole('heading', { name: /Torrents/ })).toBeHidden()
})

test('torrent screens fit a phone', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/?freeze=3')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  await expect(page.getByRole('table', { name: 'Networks in this torrent' })).toBeVisible()
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)
  expect(overflow).toBeLessThanOrEqual(0)
})

async function dropFile(page: Page, name: string, bytes: number[] | number) {
  await page.evaluate(
    ({ name, bytes }) => {
      const data = typeof bytes === 'number' ? new Uint8Array(bytes) : new Uint8Array(bytes)
      const dt = new DataTransfer()
      dt.items.add(new File([data], name, { type: 'application/x-bittorrent' }))
      window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, cancelable: true }))
    },
    { name, bytes },
  )
}

test('dropping a .torrent file opens the file picker', async ({ page }) => {
  await page.goto('/?torrents=0')
  await expect(page.getByText('Demo data')).toBeVisible()
  await dropFile(page, 'sprite-fright.torrent', [0x64, 0x34, 0x3a])
  const pick = page.getByRole('dialog', { name: 'Choose files' })
  await expect(pick).toBeVisible()
  await expect(pick.getByText('Sprite Fright (2021) 4K', { exact: true })).toBeVisible()
})

test('a damaged or oversized dropped file gets a clear error', async ({ page }) => {
  await page.goto('/?torrents=0')
  await expect(page.getByText('Demo data')).toBeVisible()
  await dropFile(page, 'broken.torrent', [0x3c, 0x68])
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await expect(dialog.getByRole('alert')).toContainText('not a valid torrent file')
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await dropFile(page, 'huge.torrent', 9 * 1024 * 1024)
  await expect(page.getByText(/a \.torrent file is at most 8 MiB/)).toBeVisible()
  await expect(page.getByRole('dialog')).toBeHidden()
})

test('a finished torrent keeps its credit but offers no file choice', async ({ page }) => {
  await page.goto('/?speed=50')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  await expect(page.getByText('Done', { exact: true }).first()).toBeVisible({ timeout: 15000 })
  await expect(page.getByRole('region', { name: 'Files' })).toBeHidden()
  const peers = await page
    .getByRole('table', { name: 'Networks in this torrent' })
    .locator('tbody tr td:nth-child(2)')
    .allTextContents()
  expect(peers.every((p) => p.trim() === '0')).toBe(true)
  await expect(
    page.getByRole('article').getByRole('button', { name: 'Pause', exact: true }),
  ).toBeHidden()
})

test("the sidebar counts torrent traffic in each network's speed", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/?empty=1&torrents=1')
  const side = page.locator('.sidebar-nets')
  await expect(side.getByText(/MB\/s/).first()).toBeVisible({ timeout: 5000 })
})

test('sharing is off by default, and its limits are checked', async ({ page }) => {
  await page.goto('/?torrents=0')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  const sw = page.getByRole('switch', { name: 'Share torrents after downloading' })
  await expect(sw).toHaveAttribute('aria-checked', 'false')
  await expect(page.getByLabel('Stop at ratio')).toBeHidden()
  await sw.click()
  await expect(sw).toHaveAttribute('aria-checked', 'true')
  const form = page.getByRole('form', { name: 'Share torrents after downloading' })
  const ratio = form.getByLabel('Stop at ratio')
  await ratio.fill('0')
  await form.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.getByText('The sharing ratio must be between 0.1 and 10.')).toBeVisible()
  await expect(ratio).toHaveAttribute('aria-invalid', 'true')
  await ratio.fill('2')
  await form.getByLabel('Stop after (minutes)').fill('45')
  await form.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(form.getByText('Saved.', { exact: true })).toBeVisible()
  await expect(ratio).not.toHaveAttribute('aria-invalid', 'true')
})

test('a finished torrent shares, then stops when asked', async ({ page }) => {
  await page.goto('/?share=1&speed=50')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  const article = page.getByRole('article')
  await expect(article.getByText('Sharing', { exact: true })).toBeVisible({ timeout: 15000 })
  await expect(article.getByText('Shared', { exact: true })).toBeVisible()
  await article.getByRole('button', { name: 'Stop sharing' }).click()
  await expect(article.getByText('Done', { exact: true })).toBeVisible()
  await expect(article.getByRole('button', { name: 'Stop sharing' })).toBeHidden()
})
