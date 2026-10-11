import { test, expect, chromium, type BrowserContext, type Worker } from '@playwright/test'
import { execFileSync, spawn, type ChildProcess } from 'node:child_process'
import { createHash, randomBytes } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
} from 'node:fs'
import { createServer, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// The whole chain with the real desktop app (BROWSER-EXTENSION.md §6, §7): the
// extension in Chromium, the host, the app's local API and its downloader. Opt-in
// (it opens the app's window): FUSELANE_DESKTOP=<path to the built fuselane-desktop>.
// Everything lives in a temporary HOME and FUSELANE_HOME, so no real browser,
// download folder or app data is touched.

declare const chrome: {
  runtime: { sendNativeMessage(host: string, message: object): Promise<unknown> }
  storage: { local: { get(k: string): Promise<Record<string, unknown>> } }
}

const desktop = process.env.FUSELANE_DESKTOP
test.skip(!desktop || process.platform === 'win32', 'set FUSELANE_DESKTOP (macOS, Linux)')

const root = resolve(import.meta.dirname, '../../..')
const extension = resolve(import.meta.dirname, '../.output/chrome-mv3')
const cli = process.env.FUSELANE_CLI ?? join(root, 'target/debug/fuselane')

let tmp: string
let app: ChildProcess
let server: Server
let base: string
let context: BrowserContext
let worker: Worker
const big = randomBytes(3 * 1024 * 1024)
const small = randomBytes(64 * 1024)
const sha = (b: Buffer) => createHash('sha256').update(b).digest('hex')

const env = () => ({
  ...process.env,
  HOME: join(tmp, 'home'),
  XDG_CONFIG_HOME: join(tmp, 'config'),
  XDG_DOWNLOAD_DIR: join(tmp, 'home/Downloads'),
  FUSELANE_HOME: join(tmp, 'data'),
})

const ask = (message: object) =>
  worker.evaluate(
    (m) => chrome.runtime.sendNativeMessage('app.fuselane.host', m).catch((e) => String(e)),
    message,
  )

test.beforeAll(async () => {
  test.setTimeout(120_000)
  tmp = mkdtempSync(join(tmpdir(), 'fuselane-app-e2e-'))
  mkdirSync(join(tmp, 'home/Downloads'), { recursive: true })
  const chromiumDir =
    process.platform === 'darwin'
      ? join(tmp, 'home/Library/Application Support/Chromium')
      : join(tmp, 'config/chromium')
  mkdirSync(chromiumDir, { recursive: true })

  // Files served with a size and ranges, like a real download site.
  server = createServer((req, res) => {
    const body = req.url?.startsWith('/big') ? big : req.url?.startsWith('/small') ? small : null
    if (!body) return void res.writeHead(404).end()
    const name = req.url!.split('/').pop()!.split('?')[0]
    const range = /bytes=(\d+)-(\d*)/.exec(req.headers.range ?? '')
    const headers = {
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': `attachment; filename="${name}"`,
      'Accept-Ranges': 'bytes',
      ETag: `"${sha(body).slice(0, 16)}"`,
    }
    if (range) {
      const start = Number(range[1])
      const end = range[2] ? Number(range[2]) : body.length - 1
      res.writeHead(206, {
        ...headers,
        'Content-Range': `bytes ${start}-${end}/${body.length}`,
        'Content-Length': end - start + 1,
      })
      return void res.end(req.method === 'HEAD' ? undefined : body.subarray(start, end + 1))
    }
    res.writeHead(200, { ...headers, 'Content-Length': body.length })
    res.end(req.method === 'HEAD' ? undefined : body)
  })
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r))
  base = `http://127.0.0.1:${(server.address() as { port: number }).port}`

  app = spawn(desktop!, [], { env: env(), stdio: 'ignore' })
  // The app is ready once it has its database; the first ping proves the API.
  const db = join(tmp, 'data/jobs.db')
  for (let i = 0; i < 200 && !existsSync(db); i++) await new Promise((r) => setTimeout(r, 150))
  expect(existsSync(db), 'the app started').toBe(true)
  await new Promise((r) => setTimeout(r, 1500))

  context = await chromium.launchPersistentContext(join(tmp, 'profile'), {
    channel: 'chromium',
    env: env(),
    acceptDownloads: true,
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  })
  worker = context.serviceWorkers()[0] ?? (await context.waitForEvent('serviceworker'))
  const host = join(tmp, 'fuselane')
  copyFileSync(cli, host)
  chmodSync(host, 0o755)
  execFileSync(host, ['browsers', '--json'], {
    env: { ...env(), FUSELANE_EXTRA_EXTENSION_IDS: new URL(worker.url()).host },
  })
  mkdirSync(join(tmp, 'profile/NativeMessagingHosts'), { recursive: true })
  copyFileSync(
    join(chromiumDir, 'NativeMessagingHosts/app.fuselane.host.json'),
    join(tmp, 'profile/NativeMessagingHosts/app.fuselane.host.json'),
  )
})

test.afterAll(async () => {
  await context?.close()
  app?.kill()
  server?.close()
  if (tmp && !process.env.KEEP_TMP) rmSync(tmp, { recursive: true, force: true })
  else console.log('kept', tmp)
})

test('a ping reaches the running app, which sends where downloads go', async () => {
  const r = (await ask({ type: 'ping', browser: 'chromium', version: '0.3.0' })) as {
    type: string
    app: { version: string; handoff: { enabled: boolean; minBytes: number } }
  }
  expect(r.type).toBe('pong')
  expect(r.app.version).toMatch(/^\d+\.\d+\.\d+/)
  expect(r.app.handoff).toEqual({ enabled: true, minBytes: 1024 * 1024 })
})

test('the setting changes in the app, and bad values are refused', async () => {
  expect(
    await ask({ v: 1, type: 'handoff.set', enabled: true, minBytes: 2 * 1024 * 1024 }),
  ).toEqual({ v: 1, type: 'handoff', handoff: { enabled: true, minBytes: 2 * 1024 * 1024 } })
  const bad = (await ask({ v: 1, type: 'handoff.set', enabled: 'yes', minBytes: -5 })) as {
    error?: string
  }
  expect(bad.error).toBeTruthy()
  const r = (await ask({ type: 'ping', browser: 'chromium', version: '0.3.0' })) as {
    app: { handoff: unknown }
  }
  expect(r.app.handoff).toEqual({ enabled: true, minBytes: 2 * 1024 * 1024 })
})

test('the app remembers which browser checked in', async () => {
  const db = join(tmp, 'data/jobs.db')
  const row = execFileSync(
    'sqlite3',
    [db, "select value from settings where key='ext_seen.chromium'"],
    {
      encoding: 'utf8',
    },
  ).trim()
  // The test loads an unpacked copy, which no store id matches.
  expect(row).toMatch(/^\d+\|\|0\.3\.0$/)
})

async function download(page: import('@playwright/test').Page, path: string) {
  await page.setContent(`<a id="go" href="${base}${path}" download>get</a>`)
  await page.click('#go')
}

test('a big download from the page goes to the app and arrives byte for byte', async () => {
  // A first launch checks its networks before downloading, which can take a while.
  test.setTimeout(150_000)
  // The extension picks up the app's setting when its popup opens.
  const popup = await context.newPage()
  await popup.goto(`chrome-extension://${new URL(worker.url()).host}/popup.html`)
  await expect(popup.locator('#chip')).toHaveText('Connected')
  await popup.close()

  const page = await context.newPage()
  await download(page, '/big/fuselane-e2e-big.bin')
  const file = join(tmp, 'home/Downloads/fuselane-e2e-big.bin')
  await expect
    .poll(() => existsSync(file) && readFileSync(file).length, { timeout: 120_000 })
    .toBe(big.length)
  expect(sha(readFileSync(file))).toBe(sha(big))
})

test('a file under the size stays in the browser (negative)', async () => {
  const page = await context.newPage()
  const browserGot = page.waitForEvent('download', { timeout: 15_000 })
  await download(page, '/small/fuselane-e2e-small.bin')
  const d = await browserGot
  expect(d.suggestedFilename()).toBe('fuselane-e2e-small.bin')
  // The app didn't take it.
  expect(readdirSync(join(tmp, 'home/Downloads'))).not.toContain('fuselane-e2e-small.bin')
})

test('with hand-off off, even a big file stays in the browser (negative)', async () => {
  await ask({ v: 1, type: 'handoff.set', enabled: false, minBytes: 2 * 1024 * 1024 })
  const popup = await context.newPage()
  await popup.goto(`chrome-extension://${new URL(worker.url()).host}/popup.html`)
  await expect(popup.getByLabel('Hand big downloads to Fuselane')).not.toBeChecked()
  await popup.close()
  const page = await context.newPage()
  const browserGot = page.waitForEvent('download', { timeout: 15_000 })
  await download(page, '/big/fuselane-e2e-off.bin')
  expect((await browserGot).suggestedFilename()).toBe('fuselane-e2e-off.bin')
  expect(readdirSync(join(tmp, 'home/Downloads'))).not.toContain('fuselane-e2e-off.bin')
})
