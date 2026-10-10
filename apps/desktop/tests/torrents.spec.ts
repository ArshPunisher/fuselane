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
  const col =
    (await table.locator('thead th').allTextContents()).findIndex((h) => h.trim() === 'Share') + 1
  const shares = await table.locator(`tbody tr td:nth-child(${col})`).allTextContents()
  expect(shares.reduce((a, s) => a + parseInt(s, 10), 0)).toBeGreaterThanOrEqual(99)
  await expect(page.locator('[data-testid="fuse-core"] .speed')).toContainText('MB/s')
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
  // A magnet with a hash shows as a card; Change reveals the link itself.
  const nd = page.getByRole('dialog', { name: 'New download' })
  await expect(nd.locator('.link-card')).toContainText('Torrent 01234567')
  await nd.getByRole('button', { name: 'Change the link' }).click()
  await expect(nd.getByLabel('Link')).toHaveValue(magnet)
})

test('files can be changed mid-download and the torrent removed keeping files', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  await page.getByRole('tab', { name: 'Files' }).click()
  const files = page.getByRole('region', { name: /^Files/ })
  // The demo torrent starts with 2 of 5; poster.jpg joins by giving it a priority.
  await files.getByRole('combobox', { name: 'Priority of poster.jpg' }).selectOption('normal')
  await expect(page.locator('.facts')).toContainText('3 of 5')
  await expect(files.locator('.files-head')).toContainText('3 of 5')
  await files.getByRole('combobox', { name: 'Priority of poster.jpg' }).selectOption('skip')
  await expect(page.locator('.facts')).toContainText('2 of 5')

  await page.getByRole('button', { name: 'Remove' }).click()
  const ask = page.getByRole('dialog', { name: 'Remove this torrent?' })
  await expect(
    ask.getByRole('button', { name: /^Move files to (Trash|Recycle Bin)$/ }),
  ).toBeVisible()
  await ask.getByRole('button', { name: 'Keep files' }).click()
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
  await expect(page.locator('[data-testid="fuse-core"] .speed')).toHaveText('100%', {
    timeout: 15000,
  })
  await expect(page.getByRole('region', { name: 'Files' })).toBeHidden()
  await expect(page.getByRole('tab', { name: 'Files' })).toHaveCount(0)
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
  // WebKit sometimes drops a fill made right after the form appears: retry it.
  await expect(async () => {
    await ratio.fill('0')
    await form.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByText('The sharing ratio must be between 0.1 and 10.')).toBeVisible({
      timeout: 1000,
    })
  }).toPass()
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
  await expect(article.getByRole('definition').filter({ hasText: 'Sharing' })).toBeVisible({
    timeout: 15000,
  })
  await expect(article.getByText('Shared', { exact: true })).toBeVisible()
  await article.getByRole('button', { name: 'Stop sharing' }).click()
  await expect(article.getByRole('definition').filter({ hasText: 'Done' })).toBeVisible()
  await expect(article.getByRole('button', { name: 'Stop sharing' })).toBeHidden()
})

test('the OS opening a magnet replaces whatever the dialog was showing', async ({ page }) => {
  await page.goto('/?torrents=0')
  const dialog = await openDialog(page)
  await dialog.getByRole('button', { name: 'Open .torrent…' }).click()
  await expect(page.getByRole('dialog', { name: 'Choose files' })).toBeVisible()
  const magnet = 'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=debian'
  await page.evaluate((m) => {
    ;(window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(m)
  }, magnet)
  const nd = page.getByRole('dialog', { name: 'New download' })
  await expect(nd.locator('.link-card-name')).toHaveText('debian')
  await nd.getByRole('button', { name: 'Next' }).click()
  await expect(
    page
      .getByRole('dialog', { name: 'Choose files' })
      .getByText('debian-13.1.0-amd64-DVD-1.iso')
      .first(),
  ).toBeVisible()
})

test('the OS opening a .torrent file goes straight to choosing files', async ({ page }) => {
  await page.goto('/?torrents=0')
  await expect(page.getByText('Demo data')).toBeVisible()
  await page.evaluate(() => {
    ;(window as unknown as { __demoOpen: (t: string) => void }).__demoOpen(
      '/Users/someone/Downloads/sprite-fright.torrent',
    )
  })
  await expect(page.getByRole('dialog', { name: 'Choose files' })).toBeVisible()
})

test('a torrent shows its pieces filling in and who it is talking to', async ({ page }) => {
  await page.goto('/')
  await page.getByText('Sprite Fright (2021) 4K').click()
  await expect(page.getByRole('img', { name: /Pieces: \d+% here/ })).toBeVisible()
  const tabs = page.getByRole('tablist', { name: 'Torrent details' })
  await expect(tabs.getByRole('tab', { name: 'Networks' })).toHaveAttribute('aria-selected', 'true')
  await tabs.getByRole('tab', { name: /Peers/ }).click()
  const peers = page.getByRole('list', { name: 'Connected peers' })
  await expect(peers.getByRole('listitem').first()).toBeVisible()
  await expect(peers).toContainText('qBittorrent 5.0.4')
  await expect(peers).toContainText('Ethernet')
  await expect(peers).toContainText('Unknown app')
  await tabs.getByRole('tab', { name: 'Files' }).click()
  await expect(page.getByRole('tabpanel')).toContainText('.mkv')
})

test('the big speed is the sum of the networks round the ring, and the sidebar says what its total counts', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  // The ring shows each network's speed beside it, like a download's.
  const split = page.locator('[data-testid="fuse-core"] .core-labels')
  await expect(split.locator('li')).toHaveCount(3)
  // Read both once the glide has settled.
  await expect
    .poll(async () => {
      const lanes = await split
        .locator('.num')
        .evaluateAll((els) =>
          els.map((e) => parseFloat(e.firstChild?.textContent ?? '0')).reduce((a, b) => a + b, 0),
        )
      const big = parseFloat(
        (await page.locator('[data-testid="fuse-core"] .speed').textContent()) ?? '0',
      )
      return Math.abs(lanes - big) <= 0.15
    })
    .toBe(true)
  // The table's Speed column ends in an "All networks" row.
  const table = page.getByRole('table', { name: 'Networks in this torrent' })
  await expect(table.getByRole('columnheader', { name: 'Speed' })).toBeVisible()
  await expect(table.locator('tfoot')).toContainText('All networks')
  // Sidebar: a total, labelled as every download together.
  await expect(page.locator('.sidebar .net-total')).toContainText('MB/s')
  await expect(page.locator('.sidebar')).toContainText('All downloads together')
})

test('torrent files have priorities, per-file progress and Play while downloading', async ({
  page,
}) => {
  await page.goto('/?freeze=3')
  await page.getByRole('button', { name: /^Sprite Fright \(2021\) 4K/ }).click()
  await page.getByRole('tab', { name: 'Files' }).click()
  const files = page.getByRole('region', { name: /^Files/ })
  const table = files.getByRole('table', { name: 'Files in this torrent' })
  await expect(table.getByRole('columnheader', { name: 'Here' })).toBeVisible()
  // High goes first: the 4K file fills before the others.
  await files
    .getByRole('combobox', { name: 'Priority of Sprite Fright 4K.mkv' })
    .selectOption('high')
  const row = table.getByRole('row', { name: /Sprite Fright 4K\.mkv/ })
  await expect(row.locator('.here')).toContainText('%')
  // Only audio and video can be played; Play gives a link for other players too.
  await expect(table.getByRole('button', { name: 'Play English.srt' })).toHaveCount(0)
  await table.getByRole('button', { name: 'Play Sprite Fright 4K.mkv' }).click()
  await expect(files.getByRole('textbox', { name: 'Stream link' })).toHaveValue(
    /^http:\/\/127\.0\.0\.1:\d+\//,
  )
  // The last wanted file can't be skipped.
  for (const n of ['Sprite Fright 1080p.mkv', 'English.srt', 'Deutsch.srt', 'poster.jpg']) {
    const sel = files.getByRole('combobox', { name: `Priority of ${n}` })
    if ((await sel.inputValue()) !== 'skip') await sel.selectOption('skip')
  }
  await files
    .getByRole('combobox', { name: 'Priority of Sprite Fright 4K.mkv' })
    .selectOption('skip')
  await expect(page.getByRole('alert').filter({ hasText: 'At least one file' })).toBeVisible()
})
