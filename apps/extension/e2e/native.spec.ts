import { test, expect, chromium, type BrowserContext, type Worker } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

// The worker's own API; these callbacks run inside the extension, not in Node.
declare const chrome: {
  runtime: { sendNativeMessage(host: string, message: object): Promise<unknown> }
}

// Linux only: Chromium finds host manifests under $XDG_CONFIG_HOME there, so the
// test can point it at a temporary folder. On macOS it would use the real
// ~/Library folders, which a test must never touch.
test.skip(process.platform !== 'linux', 'Linux only (temporary config folder)')

const root = resolve(import.meta.dirname, '../../..')
const extension = resolve(import.meta.dirname, '../.output/chrome-mv3')
const cli = process.env.FUSELANE_CLI ?? join(root, 'target/debug/fuselane')

let tmp: string
let context: BrowserContext
let worker: Worker

test.beforeAll(async () => {
  expect(existsSync(join(extension, 'manifest.json')), 'run `wxt build` first').toBe(true)
  expect(existsSync(cli), `build the CLI first (${cli})`).toBe(true)
  tmp = mkdtempSync(join(tmpdir(), 'fuselane-e2e-'))
  const config = join(tmp, 'config')
  // Chromium has "run" once: its folder exists, so `fuselane browsers` sees it.
  mkdirSync(join(config, 'chromium'), { recursive: true })
  const env = {
    ...process.env,
    HOME: join(tmp, 'home'),
    XDG_CONFIG_HOME: config,
    // The host the browser starts inherits this: a data folder with no app running.
    FUSELANE_HOME: join(tmp, 'data'),
  }
  context = await chromium.launchPersistentContext(join(tmp, 'profile'), {
    channel: 'chromium',
    env,
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
  const config = join(tmp, 'config')
  const out = execFileSync(cli, ['browsers', '--json'], {
    env: {
      ...process.env,
      HOME: join(tmp, 'home'),
      XDG_CONFIG_HOME: config,
      FUSELANE_HOME: join(tmp, 'data'),
      FUSELANE_EXTRA_EXTENSION_IDS: id,
    },
    encoding: 'utf8',
  })
  const rows = JSON.parse(out) as { browser: string; state: string }[]
  expect(rows.find((r) => r.browser === 'Chromium')?.state).toBe('registered')
  expect(rows.find((r) => r.browser === 'Chrome')?.state).toBe('not-installed')
  // Playwright's Chromium may be a Chrome for Testing build, which reads its own folder.
  const manifest = join(config, 'chromium/NativeMessagingHosts/app.fuselane.host.json')
  for (const dir of ['google-chrome-for-testing', 'google-chrome']) {
    mkdirSync(join(config, dir, 'NativeMessagingHosts'), { recursive: true })
    copyFileSync(manifest, join(config, dir, 'NativeMessagingHosts/app.fuselane.host.json'))
  }

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
