// Renders the brand SVGs to PNGs with a headless browser (one source of truth).
// Usage: node render.mjs <out-dir>
import { chromium } from '../../apps/desktop/node_modules/@playwright/test/index.mjs'
import { readFileSync } from 'node:fs'
const out = process.argv[2] ?? '.'
const b = await chromium.launch()
async function png(svgFile, size, file, bg = 'transparent') {
  const svg = readFileSync(new URL(svgFile, import.meta.url), 'utf8')
  const p = await b.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 })
  await p.setContent(
    `<html><body style="margin:0;background:${bg}">${svg.replace('<svg ', `<svg width="${size}" height="${size}" `)}</body></html>`,
  )
  await p.screenshot({ path: `${out}/${file}`, omitBackground: bg === 'transparent' })
  await p.close()
}
await png('app-icon.svg', 1024, 'app-icon-1024.png')
await png('favicon.svg', 180, 'apple-touch-icon.png', '#0a0c11')
await png('favicon.svg', 512, 'icon-512.png')
await png('favicon.svg', 192, 'icon-192.png')
await png('favicon.svg', 32, 'favicon-32.png')
await b.close()

// Share card (Open Graph): logo, headline and the real app screenshot.
{
  const b2 = await chromium.launch()
  const p = await b2.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 })
  const mark = readFileSync(new URL('mark.svg', import.meta.url), 'utf8')
  // Inline as data URLs: a page set from a string can't load local files.
  const font =
    'data:font/woff2;base64,' +
    readFileSync(
      new URL(
        '../../apps/site-next/node_modules/geist/dist/fonts/geist-sans/Geist-Variable.woff2',
        import.meta.url,
      ),
    ).toString('base64')
  const shot =
    'data:image/png;base64,' +
    readFileSync(new URL('../../apps/site-next/public/shots/app.png', import.meta.url)).toString(
      'base64',
    )
  await p.setContent(`<html><head><style>
    @font-face { font-family: G; src: url('${font}'); font-weight: 100 900; }
    body { margin:0; width:1200px; height:630px; background:#0a0c11; font-family:G, sans-serif; overflow:hidden; position:relative; }
    .l { position:absolute; left:72px; top:80px; width:520px; }
    .brand { display:flex; align-items:center; gap:14px; color:#f2f3f6; font-size:34px; font-weight:650; letter-spacing:-0.02em; }
    .brand svg { width:64px; height:64px; }
    h1 { margin:64px 0 0; color:#f2f3f6; font-size:76px; line-height:1.02; letter-spacing:-0.035em; font-weight:650; }
    p { margin:28px 0 0; color:#c1c4c9; font-size:26px; line-height:1.4; }
    .s { position:absolute; left:640px; top:96px; width:760px; border-radius:16px; border:1px solid rgba(255,255,255,0.16); box-shadow:0 30px 80px rgba(0,0,0,0.6); }
  </style></head><body>
    <div class="l"><div class="brand">${mark}<span>Fuselane</span></div>
    <h1>One download.<br>Every network.</h1>
    <p>Free download manager for macOS, Windows and Linux.</p></div>
    <img class="s" src="${shot}">
  </body></html>`)
  await p.waitForTimeout(500)
  await p.screenshot({ path: `${out}/og.png` })
  await b2.close()
}
// 16 px favicon for the .ico
{
  const b3 = await chromium.launch()
  const p = await b3.newPage({ viewport: { width: 16, height: 16 } })
  const svg = readFileSync(new URL('favicon.svg', import.meta.url), 'utf8').replace(
    '<svg ',
    '<svg width="16" height="16" ',
  )
  await p.setContent(`<html><body style="margin:0">${svg}</body></html>`)
  await p.screenshot({ path: `${out}/favicon-16.png`, omitBackground: true })
  await b3.close()
}
