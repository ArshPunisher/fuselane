import { test, expect, chromium, type BrowserContext, type Worker } from '@playwright/test'
import { createHash } from 'node:crypto'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { createServer, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// Without the app (REVIEW-2026-10-11 D): a clicked file link is taken before the
// browser starts it and the extension downloads it itself, in parts, into the
// Downloads folder; a file over the limit asks "Get the app or the browser".
// Uses the test build (FUSELANE_E2E=1 wxt build): site access granted at load.
const extension = resolve(import.meta.dirname, '../.output-e2e/chrome-mv3')

/** The parts of chrome.* these tests use, inside the extension's service worker. */
interface DownloadItem {
  id: number
  url: string
  filename: string
  state: string
  byExtensionId?: string
}
declare const chrome: {
  storage: {
    local: { get(k: string): Promise<Record<string, unknown>>; set(v: object): Promise<void> }
  }
  downloads: { search(q: object): Promise<DownloadItem[]> }
  scripting: { getRegisteredContentScripts(): Promise<unknown[]> }
}

/** Byte i of every test file: anything deterministic, so no file is kept in memory. */
const byte = (i: number) => (i * 131 + (i >> 12)) & 0xff
function bytes(start: number, end: number): Buffer {
  const b = Buffer.alloc(end - start + 1)
  for (let i = start; i <= end; i++) b[i - start] = byte(i)
  return b
}
const SIZES: Record<string, number> = {
  '/files/tool.zip': 9 * 1024 * 1024 + 77,
  '/files/huge.iso': 150 * 1024 * 1024,
  '/files/script.bin': 6 * 1024 * 1024 + 5,
  '/files/slow.bin': 24 * 1024 * 1024 + 9,
  '/files/slow-two.bin': 24 * 1024 * 1024 + 11,
}
/** slow.bin trickles (about 4 MB/s in all) so a pause lands mid-way. */
const SLOW = '/files/slow.bin'
/** Trickles too (its own link: one taken seconds ago isn't taken again). */
const isSlow = (path: string) => path.startsWith('/files/slow')

let server: Server
let base = ''
let tmp: string
let context: BrowserContext
let worker: Worker
let id = ''

test.beforeAll(async () => {
  expect(existsSync(join(extension, 'manifest.json')), 'run `FUSELANE_E2E=1 wxt build` first').toBe(
    true,
  )
  server = createServer((req, res) => {
    const path = (req.url ?? '/').split('?')[0]!
    const size = SIZES[path]
    if (!size) {
      res.writeHead(200, { 'content-type': 'text/html' })
      res.end(
        `<!doctype html><title>files</title>` +
          `<a id="tool" href="/files/tool.zip">tool.zip</a> <a id="huge" href="/files/huge.iso">huge.iso</a> ` +
          `<a id="slow" href="/files/slow.bin">slow.bin</a> ` +
          `<a id="slow2" href="/files/slow-two.bin">slow-two.bin</a> ` +
          `<button id="script" onclick="location.href='/files/script.bin'">script</button>`,
      )
      return
    }
    const m = /bytes=(\d+)-(\d*)/.exec(req.headers.range ?? '')
    if (m) {
      const start = Number(m[1])
      const end = Math.min(size - 1, m[2] ? Number(m[2]) : size - 1)
      res.writeHead(206, {
        'content-type': 'application/octet-stream',
        'content-range': `bytes ${start}-${end}/${size}`,
        'content-length': String(end - start + 1),
        'accept-ranges': 'bytes',
      })
      // In pieces, so big ranges stream; slow.bin waits between them.
      const piece = isSlow(path) ? 64 * 1024 : 1 << 20
      void (async () => {
        for (let s = start; s <= end && !res.destroyed; s += piece) {
          res.write(bytes(s, Math.min(end, s + piece - 1)))
          if (isSlow(path)) await new Promise((r) => setTimeout(r, 60))
        }
        res.end()
      })()
      return
    }
    res.writeHead(200, {
      'content-type': 'application/octet-stream',
      'content-length': String(size),
      'accept-ranges': 'bytes',
    })
    if (req.method === 'HEAD') return res.end()
    for (let s = 0; s < size; s += 1 << 20)
      res.write(bytes(s, Math.min(size - 1, s + (1 << 20) - 1)))
    res.end()
  })
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r))
  const addr = server.address()
  base = `http://127.0.0.1:${typeof addr === 'object' && addr ? addr.port : 0}`
  tmp = mkdtempSync(join(tmpdir(), 'fuselane-self-'))
  context = await chromium.launchPersistentContext(join(tmp, 'profile'), {
    channel: 'chromium',
    acceptDownloads: true,
    downloadsPath: join(tmp, 'downloads'),
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  })
  worker = context.serviceWorkers()[0] ?? (await context.waitForEvent('serviceworker'))
  id = new URL(worker.url()).host
  // The link script is registered once site access is known; wait for it.
  await expect
    .poll(
      () =>
        worker.evaluate(async () => (await chrome.scripting.getRegisteredContentScripts()).length),
      { timeout: 10_000 },
    )
    .toBe(1)
})

test.afterAll(async () => {
  await context?.close()
  server?.close()
  if (tmp) rmSync(tmp, { recursive: true, force: true })
})

const jobs = () =>
  worker.evaluate(
    async () =>
      ((await chrome.storage.local.get('selfJobs')).selfJobs ?? []) as {
        name: string
        status: string
        savedId: number | null
        error: string | null
      }[],
  )

test('a clicked file link downloads in the extension, byte for byte, and never in the browser', async () => {
  const page = await context.newPage()
  await page.goto(base)
  await page.click('#tool')
  await expect
    .poll(
      async () => {
        const j = (await jobs()).find((x) => x.name === 'tool.zip')
        return j?.status === 'failed' ? `failed: ${j.error}` : j?.status
      },
      { timeout: 30_000 },
    )
    .toBe('completed')
  const job = (await jobs()).find((j) => j.name === 'tool.zip')!
  // The only browser download is the extension's own save of the finished file.
  const items = await worker.evaluate(() => chrome.downloads.search({}))
  expect(items).toHaveLength(1)
  expect(items[0]!.id).toBe(job.savedId)
  expect(items[0]!.byExtensionId).toBe(id)
  expect(items[0]!.state).toBe('complete')
  const saved = readFileSync(items[0]!.filename)
  const want = createHash('sha256')
    .update(bytes(0, SIZES['/files/tool.zip']! - 1))
    .digest('hex')
  expect(createHash('sha256').update(saved).digest('hex')).toBe(want)
  await page.close()
})

test('a file over the limit asks in the popup: get the app, or download in the browser', async () => {
  await worker.evaluate(() => chrome.storage.local.set({ selfLimit: 100 * 1024 * 1024 }))
  const page = await context.newPage()
  await page.goto(base)
  await page.click('#huge')
  // The question waits in the popup (opened by itself), with a "!" on the icon.
  await expect
    .poll(() =>
      worker.evaluate(async () => {
        const a = ((await chrome.storage.local.get('bigAsks')).bigAsks as { name?: string }[])?.[0]
        const badge = await (
          chrome as unknown as { action: { getBadgeText(d: object): Promise<string> } }
        ).action.getBadgeText({})
        return `${a?.name ?? ''} ${badge}`
      }),
    )
    .toBe('huge.iso !')
  const popup = await context.newPage()
  await popup.goto(`chrome-extension://${id}/popup.html`)
  const ask = popup.locator('.card-ask')
  await expect(ask.getByText('This one needs the Fuselane app')).toBeVisible()
  await expect(ask.getByText(/huge\.iso is 150 MB/)).toBeVisible()
  await ask.getByRole('button', { name: 'Download in the browser' }).click()
  // Now the browser has it, as asked (not taken again by Fuselane), and the
  // question is gone.
  await expect
    .poll(
      async () =>
        (await worker.evaluate(() => chrome.downloads.search({}))).filter((i: DownloadItem) =>
          i.url.endsWith('/files/huge.iso'),
        ).length,
      { timeout: 15_000 },
    )
    .toBe(1)
  expect((await jobs()).find((j) => j.name === 'huge.iso')?.status).toBe('too-big')
  await expect
    .poll(() =>
      worker.evaluate(async () => (await chrome.storage.local.get('bigAsk')).bigAsk ?? null),
    )
    .toBe(null)
})

test('a download a page starts by script is taken from the browser without a trace', async () => {
  const page = await context.newPage()
  await page.goto(base)
  await page.click('#script')
  await expect
    .poll(async () => (await jobs()).find((x) => x.name === 'script.bin')?.status, {
      timeout: 30_000,
    })
    .toBe('completed')
  const items = await worker.evaluate(() => chrome.downloads.search({}))
  // Only the extension's saves: the browser's own copy was cancelled and erased.
  expect(items.filter((i: DownloadItem) => i.url.endsWith('/files/script.bin'))).toHaveLength(0)
  const job = (await jobs()).find((x) => x.name === 'script.bin')!
  const saved = items.find((i: DownloadItem) => i.id === job.savedId)!
  const want = createHash('sha256')
    .update(bytes(0, SIZES['/files/script.bin']! - 1))
    .digest('hex')
  expect(createHash('sha256').update(readFileSync(saved.filename)).digest('hex')).toBe(want)
  await page.close()
})

test('a download pauses, keeps what it has, and resumes to the exact file', async () => {
  const page = await context.newPage()
  await page.goto(base)
  await page.click('#slow')
  const find = async () =>
    (await jobs()).find((x) => x.name === 'slow.bin') as
      { id: string; status: string; written: number } | undefined
  await expect
    .poll(async () => (await find())?.written ?? 0, { timeout: 20_000 })
    .toBeGreaterThan(1024 * 1024)
  const id = (await find())!.id
  // The popup's buttons send these; an extension page sends them the same way.
  const ext = await context.newPage()
  await ext.goto(`chrome-extension://${await worker.evaluate(() => location.host)}/decide.html`)
  const send = (action: string) =>
    ext.evaluate(
      ([jobId, a]) =>
        (
          chrome as unknown as { runtime: { sendMessage(m: object): Promise<void> } }
        ).runtime.sendMessage({ type: 'self.action', id: jobId, action: a }),
      [id, action],
    )
  await send('pause')
  await expect.poll(async () => (await find())?.status, { timeout: 10_000 }).toBe('paused')
  const kept = (await find())!.written
  expect(kept).toBeGreaterThan(0)
  expect(kept).toBeLessThan(SIZES[SLOW]!)
  await page.waitForTimeout(800)
  expect((await find())!.written).toBe(kept)
  await send('resume')
  await expect.poll(async () => (await find())?.status, { timeout: 60_000 }).toBe('completed')
  const job = (await jobs()).find((x) => x.name === 'slow.bin')!
  const saved = (await worker.evaluate(() => chrome.downloads.search({}))).find(
    (i: DownloadItem) => i.id === job.savedId,
  )!
  const want = createHash('sha256')
    .update(bytes(0, SIZES[SLOW]! - 1))
    .digest('hex')
  expect(createHash('sha256').update(readFileSync(saved.filename)).digest('hex')).toBe(want)
  await page.close()
})

test('cancelling a download asks first, in its row', async () => {
  const page = await context.newPage()
  await page.goto(base)
  await page.click('#slow2')
  await expect
    .poll(
      async () => (await jobs()).find((x) => x.name === 'slow-two.bin' && x.status === 'running'),
      {
        timeout: 20_000,
      },
    )
    .toBeTruthy()
  const popup = await context.newPage()
  await popup.goto(`chrome-extension://${id}/popup.html`)
  const row = popup.locator('li.job', { hasText: 'slow-two.bin' }).first()
  await row.getByRole('button', { name: 'Cancel: slow-two.bin' }).click()
  const ask = row.getByRole('group', { name: 'Cancel this download?' })
  await expect(ask).toContainText('What’s downloaded is deleted.')
  // Keep it: nothing happens, the buttons come back.
  await ask.getByRole('button', { name: 'Keep it' }).click()
  await expect(ask).toHaveCount(0)
  expect((await jobs()).some((x) => x.name === 'slow-two.bin' && x.status !== 'cancelled')).toBe(
    true,
  )
  // Cancel download: it goes from the list (not finished, not failed: gone).
  await row.getByRole('button', { name: 'Cancel: slow-two.bin' }).click()
  await row
    .getByRole('group', { name: 'Cancel this download?' })
    .getByRole('button', { name: 'Cancel download' })
    .click()
  await expect
    .poll(async () => (await jobs()).filter((x) => x.name === 'slow-two.bin').length, {
      timeout: 10_000,
    })
    .toBe(0)
  await popup.close()
  await page.close()
})
