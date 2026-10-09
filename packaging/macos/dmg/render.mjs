// Renders background.html to background.png (660x420) and background@2x.png,
// then joins them into background.tiff so Finder shows it crisp on Retina.
// Run from the repo root: node packaging/macos/dmg/render.mjs
import { chromium } from '../../../apps/site/node_modules/@playwright/test/index.mjs'
import { readFileSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const font = resolve(
  here,
  '../../../apps/desktop/node_modules/geist/dist/fonts/geist-sans/Geist-Variable.woff2',
)
const html = readFileSync(resolve(here, 'background.html'), 'utf8').replace(
  'GEIST_URL',
  pathToFileURL(font).href,
)
const tmp = resolve(here, '.render.html')
writeFileSync(tmp, html)
const browser = await chromium.launch()
for (const [scale, name] of [
  [1, 'background.png'],
  [2, 'background@2x.png'],
]) {
  const page = await browser.newPage({
    viewport: { width: 660, height: 420 },
    deviceScaleFactor: scale,
  })
  await page.goto(pathToFileURL(tmp).href)
  await page.evaluate(() => document.fonts.ready)
  await page.screenshot({ path: resolve(here, name) })
}
await browser.close()
execFileSync('rm', [tmp])
execFileSync('tiffutil', [
  '-cathidpicheck',
  resolve(here, 'background.png'),
  resolve(here, 'background@2x.png'),
  '-out',
  resolve(here, 'background.tiff'),
])
console.log('rendered background.tiff')
