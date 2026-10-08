// The browser extension's icons (16, 32, 48, 128 px) from the same favicon source.
// Usage: node packaging/brand/render-extension.mjs
import { chromium } from '../../apps/site/node_modules/@playwright/test/index.mjs'
import { mkdirSync, readFileSync } from 'node:fs'
const out = new URL('../../apps/extension/public/icon/', import.meta.url)
mkdirSync(out, { recursive: true })
const svg = readFileSync(new URL('favicon.svg', import.meta.url), 'utf8')
const b = await chromium.launch()
for (const size of [16, 32, 48, 128]) {
  const p = await b.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 })
  await p.setContent(
    `<html><body style="margin:0;background:transparent">${svg.replace('<svg ', `<svg width="${size}" height="${size}" `)}</body></html>`,
  )
  await p.screenshot({ path: new URL(`${size}.png`, out).pathname, omitBackground: true })
  await p.close()
}
await b.close()
