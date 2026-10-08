import { test, expect, chromium, type BrowserContext, type Worker } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// The worker's own API; these callbacks run inside the extension, not in Node.
declare const chrome: {
  runtime: { sendNativeMessage(host: string, message: object): Promise<unknown> }
}

// Windows registers through the registry, which a test must not touch. On macOS
// and Linux everything lives in a temporary HOME: `fuselane browsers` writes
// there, and Chromium reads host manifests from its own profile folder.
test.skip(process.platform === 'win32', 'Windows registers through the registry')

const root = resolve(import.meta.dirname, '../../..')
const extension = resolve(import.meta.dirname, '../.output/chrome-mv3')
const cli = process.env.FUSELANE_CLI ?? join(root, 'target/debug/fuselane')

let tmp: string
/** A copy of the CLI outside the repo: macOS blocks a browser from running a
 *  program inside ~/Documents (a privacy-protected folder) until someone allows it. */
let host: string

// Where `fuselane browsers` looks for Chromium inside the temporary HOME.
const chromiumDir = () =>
  process.platform === 'darwin'
    ? join(tmp, 'home/Library/Application Support/Chromium')
    : join(tmp, 'config/chromium')

const env = (extra: Record<string, string> = {}) => ({
  ...process.env,
  HOME: join(tmp, 'home'),
  XDG_CONFIG_HOME: join(tmp, 'config'),
  // The host the browser starts inherits this: a data folder with no app running.
  FUSELANE_HOME: join(tmp, 'data'),
  ...extra,
})
let context: BrowserContext
let worker: Worker

test.beforeAll(async () => {
  expect(existsSync(join(extension, 'manifest.json')), 'run `wxt build` first').toBe(true)
  expect(existsSync(cli), `build the CLI first (${cli})`).toBe(true)
  tmp = mkdtempSync(join(tmpdir(), 'fuselane-e2e-'))
  host = join(tmp, process.platform === 'win32' ? 'fuselane.exe' : 'fuselane')
  copyFileSync(cli, host)
  chmodSync(host, 0o755)
  // Chromium has "run" once: its folder exists, so `fuselane browsers` sees it.
  mkdirSync(chromiumDir(), { recursive: true })
  context = await chromium.launchPersistentContext(join(tmp, 'profile'), {
    channel: 'chromium',
    env: env(),
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  })
  worker = context.serviceWorkers()[0] ?? (await context.waitForEvent('serviceworker'))
})

test.afterAll(async () => {
  await context?.close()
  if (tmp) rmSync(tmp, { recursive: true, force: true })
})

const ping = () =>
  worker.evaluate(async () => {
    try {
      return {
        reply: await chrome.runtime.sendNativeMessage('app.fuselane.host', { type: 'ping' }),
      }
    } catch (e) {
      return { error: String(e) }
    }
  })

test('without the manifest the browser says the host is missing', async () => {
  const r = await ping()
  expect(r.error).toMatch(/native messaging host not found/i)
})

test('after `fuselane browsers`, the extension reaches the host and hears the app is closed', async () => {
  const id = new URL(worker.url()).host
  const out = execFileSync(host, ['browsers', '--json'], {
    env: env({ FUSELANE_EXTRA_EXTENSION_IDS: id }),
    encoding: 'utf8',
  })
  const rows = JSON.parse(out) as { browser: string; state: string }[]
  expect(rows.find((r) => r.browser === 'Chromium')?.state).toBe('registered')
  expect(rows.find((r) => r.browser === 'Chrome')?.state).toBe('not-installed')
  // A browser started with its own profile folder (as Playwright does) reads host
  // manifests from inside it, so the written manifest is copied there.
  const manifest = join(chromiumDir(), 'NativeMessagingHosts/app.fuselane.host.json')
  mkdirSync(join(tmp, 'profile/NativeMessagingHosts'), { recursive: true })
  copyFileSync(manifest, join(tmp, 'profile/NativeMessagingHosts/app.fuselane.host.json'))

  const r = await ping()
  expect(r.error).toBeUndefined()
  expect(r.reply).toMatchObject({ v: 1, type: 'pong' })
  expect((r.reply as { error?: string }).error).toMatch(/isn't running/)
})

test('an offer with the app closed is declined, so the browser keeps the download', async () => {
  const r = await worker.evaluate(() =>
    chrome.runtime.sendNativeMessage('app.fuselane.host', {
      v: 1,
      type: 'download.offer',
      url: 'https://example.org/big.iso',
      source: 'auto',
    }),
  )
  expect(r).toMatchObject({ type: 'download.declined', fallback: 'browser' })
})
