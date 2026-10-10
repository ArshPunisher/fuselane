// Store images from the real UI (Chrome Web Store, Edge and Firefox Add-ons):
// five 1280x800 screenshots with a headline each, the small and marquee promo
// tiles, and a logo kit from the approved mark (packaging/brand/mark.svg).
// Needs the desktop demo on :5191 (`pnpm --filter @fuselane/desktop dev -- --port 5191`)
// and a built extension (`pnpm --filter @fuselane/extension build`).
// Usage: node apps/extension/store/render.mjs   (writes into apps/extension/store/out)
import { readFileSync, mkdirSync } from 'node:fs'
import { chromium } from '../../site/node_modules/@playwright/test/index.mjs'

const here = new URL('./', import.meta.url).pathname
const root = new URL('../../../', import.meta.url).pathname
const out = `${here}out/`
mkdirSync(out, { recursive: true })
const APP = process.env.APP ?? 'http://localhost:5191'
const b64 = (p) => readFileSync(p).toString('base64')
const font = b64(`${root}apps/desktop/dist/assets/Geist-Variable-Bj2R_7yk.woff2`)
const mark = readFileSync(`${root}packaging/brand/mark.svg`, 'utf8')
const svgUri = (s) => `data:image/svg+xml;base64,${Buffer.from(s).toString('base64')}`
const markUri = svgUri(mark)
const iconUri = svgUri(readFileSync(`${root}packaging/brand/app-icon.svg`, 'utf8'))

const BASE = `
  @font-face { font-family: Geist; src: url(data:font/woff2;base64,${font}) format('woff2'); font-weight: 100 900; }
  * { box-sizing: border-box; margin: 0; }
  body { font-family: Geist, system-ui, sans-serif; color: #eef0f5; -webkit-font-smoothing: antialiased; }
  .bg { background: radial-gradient(circle at 18% 12%, rgba(253,133,55,.16), transparent 42%),
                    radial-gradient(circle at 92% 100%, rgba(67,202,231,.10), transparent 45%), #0b0d12; }
  .lead { color: #aab0bf; }
`

const b = await chromium.launch()

/** A real screen of the app (demo build) as base64 PNG, after `steps`. */
async function appShot(url, steps = async () => {}) {
  const p = await b.newPage({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  })
  await p.goto(APP + url)
  // The store shows the app as people see it: no "Demo data" badge.
  await p.addStyleTag({
    content: '.chip[title^="Running in a browser"] { display: none !important; }',
  })
  await p.waitForTimeout(900)
  await steps(p)
  await p.waitForTimeout(700)
  const png = await p.screenshot()
  await p.close()
  return png.toString('base64')
}

/** The extension's built popup with a stand-in browser API listing a page's files. */
async function popupShot() {
  const dir = `${root}apps/extension/.output/chrome-mv3/`
  const p = await b.newPage({
    viewport: { width: 360, height: 390 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  })
  await p.route('http://popup.local/**', async (route) => {
    const path = new URL(route.request().url()).pathname.slice(1) || 'popup.html'
    let body = readFileSync(dir + path)
    if (path === 'popup.html')
      body = Buffer.from(
        body
          .toString()
          .replace('<head>', `<head><script>${readFileSync(`${here}stub.js`, 'utf8')}</script>`),
      )
    const type = path.endsWith('.js')
      ? 'text/javascript'
      : path.endsWith('.css')
        ? 'text/css'
        : 'text/html'
    await route.fulfill({ body, contentType: type })
  })
  await p.goto('http://popup.local/popup.html')
  await p.waitForTimeout(900)
  const png = await p.screenshot()
  await p.close()
  return png.toString('base64')
}

/** A store screenshot: headline and a line on the left, the screen on the right. */
async function frame(name, headline, line, shot, opts = {}) {
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } })
  const w = opts.width ?? 780
  await p.setContent(`<html><head><style>${BASE}
    body { width: 1280px; height: 800px; display: grid; grid-template-columns: 400px 1fr; align-items: center; overflow: hidden; }
    .text { padding: 0 24px 0 64px; display: grid; gap: 20px; }
    .logo { display: flex; align-items: center; gap: 10px; font-weight: 600; font-size: 18px; }
    .logo img { width: 30px; height: 30px; }
    h1 { font-size: 46px; line-height: 1.05; letter-spacing: -0.03em; font-weight: 650; }
    h1 em { font-style: normal; color: #fd8537; }
    p { font-size: 19px; line-height: 1.5; }
    .shot { justify-self: ${opts.center ? 'center' : 'start'}; width: ${w}px; border-radius: 14px; overflow: hidden;
      border: 1px solid rgba(255,255,255,.09); box-shadow: 0 40px 90px rgba(0,0,0,.55); }
    .shot img { display: block; width: 100%; }
  </style></head><body class="bg">
    <div class="text">
      <div class="logo"><img src="${markUri}" alt="">Fuselane</div>
      <h1>${headline}</h1>
      <p class="lead">${line}</p>
    </div>
    <div class="shot"><img src="data:image/png;base64,${shot}"></div>
  </body></html>`)
  await p.waitForTimeout(300)
  await p.screenshot({ path: `${out}${name}.jpg`, type: 'jpeg', quality: 92 })
  await p.close()
}

// 1. Every network at once: the Fuse Core.
await frame(
  'screenshot-1-every-network',
  'One download.<br><em>Every network.</em>',
  'Fuselane splits each big download across Wi-Fi, Ethernet and your phone at once, then fuses the parts into one verified file.',
  await appShot('/?freeze=6&torrents=0'),
  { width: 860 },
)

// 2. The popup: the page's videos and files, one click away.
await frame(
  'screenshot-2-from-the-browser',
  'Big downloads go <em>straight to Fuselane</em>',
  "One click gets the page's video, or any file on it. Anything the app can't take stays in the browser, so nothing is lost.",
  await popupShot(),
  { width: 360, center: true },
)

// 3. Right-click any link.
{
  const p = await b.newPage({ viewport: { width: 760, height: 520 }, deviceScaleFactor: 2 })
  await p.setContent(`<html><head><style>${BASE}
    body { width: 760px; height: 520px; background: #f6f7f9; color: #1d2230; font-size: 15px; position: relative; overflow: hidden; }
    .bar { height: 44px; background: #e9ebef; border-bottom: 1px solid #d6d9df; display: flex; align-items: center; padding: 0 16px; gap: 8px; }
    .dot { width: 11px; height: 11px; border-radius: 50%; background: #c9ccd3; }
    .url { margin-left: 18px; flex: 1; height: 28px; border-radius: 8px; background: #fff; border: 1px solid #d6d9df; display: flex; align-items: center; padding: 0 12px; color: #5b6172; font-size: 13px; }
    .page { padding: 34px 44px; display: grid; gap: 12px; }
    .page h2 { font-size: 24px; letter-spacing: -0.02em; }
    .page a { color: #2457d6; text-decoration: underline; }
    .menu { position: absolute; left: 300px; top: 205px; width: 270px; background: #fff; border: 1px solid #d0d4db; border-radius: 10px;
      box-shadow: 0 18px 40px rgba(20,25,40,.18); padding: 6px; font-size: 14px; }
    .menu div { padding: 8px 12px; border-radius: 6px; color: #3a4050; }
    .menu .sep { height: 1px; padding: 0; margin: 5px 6px; background: #e3e6eb; }
    .menu .hit { background: #fd8537; color: #fff; display: flex; align-items: center; gap: 8px; font-weight: 550; }
    .menu .hit img { width: 18px; height: 18px; background: #fff; border-radius: 4px; padding: 2px; }
  </style></head><body>
    <div class="bar"><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="url">releases.example.org/26.04</span></div>
    <div class="page">
      <h2>Desktop image</h2>
      <p>For 64-bit PCs. <a>ubuntu-26.04-desktop-amd64.iso</a> (5.8 GB)</p>
      <p>Checksums: <a>SHA256SUMS</a></p>
    </div>
    <div class="menu">
      <div>Open link in new tab</div><div>Open link in new window</div><div class="sep"></div>
      <div>Save link as…</div><div>Copy link address</div><div class="sep"></div>
      <div class="hit"><img src="${markUri}" alt="">Download with Fuselane</div>
    </div>
  </body></html>`)
  const menu = (await p.screenshot()).toString('base64')
  await p.close()
  await frame(
    'screenshot-3-right-click',
    'Right-click <em>any link</em>',
    'Choose “Download with Fuselane”. It starts at once in the app, over every network you have.',
    menu,
    { width: 760, center: true },
  )
}

// 4. Verified, and what each network saved.
await frame(
  'screenshot-4-verified',
  'Checked, and <em>faster</em>',
  'Every finished file is verified against its published checksum, and Fuselane shows how much time each network saved.',
  await appShot('/?freeze=6&torrents=0&drop=0', async (p) => {
    await p
      .getByRole('button', { name: /Blender-5\.1/ })
      .first()
      .click()
  }),
  { width: 860 },
)

// 5. Find every file on a page, get them as one group.
await frame(
  'screenshot-5-whole-page',
  'Grab every file <em>on a page</em>',
  'Paste a downloads page, pick files by type, and Fuselane fetches them together as one group with one progress bar.',
  await appShot('/?freeze=6&torrents=0&empty=1', async (p) => {
    await p.getByRole('button', { name: 'New download' }).first().click()
    await p.getByRole('dialog').getByLabel('Link').fill('https://releases.example.org/26.04/')
    await p.getByRole('button', { name: 'Find files on this page' }).click()
    await p.waitForTimeout(500)
    await p.getByRole('button', { name: /^Disk images/ }).click()
    await p.getByRole('checkbox', { name: /release-notes/ }).check()
  }),
  { width: 860 },
)

/** A promo tile or logo from HTML, at its exact size. */
async function render(name, w, h, html, type = 'jpeg') {
  const p = await b.newPage({ viewport: { width: w, height: h } })
  await p.setContent(
    `<html><head><style>${BASE} body { width:${w}px; height:${h}px; overflow:hidden; }</style></head><body>${html}</body></html>`,
  )
  await p.waitForTimeout(200)
  await p.screenshot(
    type === 'png'
      ? { path: `${out}${name}.png`, omitBackground: true }
      : { path: `${out}${name}.jpg`, type: 'jpeg', quality: 94 },
  )
  await p.close()
}

// Promo tiles.
await render(
  'promo-small-440x280',
  440,
  280,
  `<div class="bg" style="height:100%;display:grid;place-content:center;justify-items:center;gap:14px;text-align:center">
    <img src="${markUri}" style="width:96px;height:96px">
    <div style="font-size:38px;font-weight:650;letter-spacing:-0.03em">Fuselane</div>
    <div class="lead" style="font-size:17px">One download. Every network.</div>
  </div>`,
)
const core = await appShot('/?freeze=6&torrents=0')
await render(
  'promo-marquee-1400x560',
  1400,
  560,
  `<div class="bg" style="height:100%;display:grid;grid-template-columns:560px 1fr;align-items:center;overflow:hidden">
    <div style="padding-left:80px;display:grid;gap:18px">
      <div style="display:flex;align-items:center;gap:14px;font-size:26px;font-weight:600"><img src="${markUri}" style="width:46px">Fuselane</div>
      <div style="font-size:54px;line-height:1.02;font-weight:650;letter-spacing:-0.035em">One download.<br><span style="color:#fd8537">Every network.</span></div>
      <div class="lead" style="font-size:20px;line-height:1.45">Wi-Fi, Ethernet and your phone, fused into one fast download. Free and open source.</div>
    </div>
    <div style="width:980px;border-radius:16px;overflow:hidden;border:1px solid rgba(255,255,255,.09);box-shadow:0 40px 90px rgba(0,0,0,.6);transform:translateY(40px)">
      <img src="data:image/png;base64,${core}" style="display:block;width:100%">
    </div>
  </div>`,
)

// Logo kit, from the approved mark.
const lockup = (color) =>
  `<div style="height:100%;display:flex;align-items:center;justify-content:center;gap:28px">
    <img src="${markUri}" style="width:150px;height:150px">
    <span style="font-size:118px;font-weight:650;letter-spacing:-0.04em;color:${color}">Fuselane</span>
  </div>`
await render(
  'logo-lockup-on-dark',
  1000,
  300,
  `<div class="bg" style="height:100%">${lockup('#eef0f5')}</div>`,
  'png',
)
await render(
  'logo-lockup-on-light',
  1000,
  300,
  `<div style="height:100%;background:#fff">${lockup('#11141b')}</div>`,
  'png',
)
await render('logo-lockup-transparent', 1000, 300, lockup('#11141b'), 'png')
await render(
  'logo-icon-512',
  512,
  512,
  `<img src="${iconUri}" style="width:512px;height:512px">`,
  'png',
)
await render(
  'logo-icon-300',
  300,
  300,
  `<img src="${iconUri}" style="width:300px;height:300px">`,
  'png',
)
await render(
  'logo-mark-white',
  512,
  512,
  `<img src="${svgUri(mark.replace(/stroke="#[0-9a-f]{6}"/g, 'stroke="#ffffff"'))}" style="width:512px">`,
  'png',
)
await render(
  'social-avatar-800',
  800,
  800,
  `<div class="bg" style="height:100%;display:grid;place-items:center"><img src="${markUri}" style="width:470px"></div>`,
)
await b.close()
console.log('wrote', out)
