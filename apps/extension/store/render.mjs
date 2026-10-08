// Store screenshots (1280x800) from the real UI: the app (demo build) and the
// extension's built popup with a stand-in browser API. Needs the desktop demo on
// :5190 and `python3 -m http.server 5191` in apps/extension/.output/chrome-mv3.
// Usage: node apps/extension/store/render.mjs
import { chromium } from '../../site/node_modules/@playwright/test/index.mjs'
const out = new URL('./', import.meta.url).pathname
const b = await chromium.launch()

// 1. The app fusing a download over three networks.
{
  const p = await b.newPage({ viewport: { width: 1280, height: 800 }, colorScheme: 'dark' })
  await p.goto('http://localhost:5190/?freeze=6&torrents=0')
  await p.getByTestId('fuse-core').waitFor()
  await p.waitForTimeout(400)
  await p.screenshot({ path: `${out}screenshot-1-app.png` })
  await p.close()
}

// 2. The popup, connected, on the brand background.
{
  const p = await b.newPage({ viewport: { width: 1280, height: 800 }, colorScheme: 'dark' })
  await p.setContent(`<html><body style="margin:0;height:800px;display:grid;place-items:center;
    background:radial-gradient(circle at 30% 20%, #1d2333, #0a0c11 70%);font-family:system-ui;color:#e8eaf0">
    <div style="display:grid;gap:28px;justify-items:center">
      <p style="margin:0;font-size:30px;font-weight:600;letter-spacing:-0.01em">Big downloads go to Fuselane</p>
      <iframe src="http://localhost:5191/popup.html" style="width:332px;height:200px;border:1px solid #2a3040;border-radius:14px;background:#1b1f2a"></iframe>
      <p style="margin:0;font-size:18px;color:#a3a9b8">Anything Fuselane can't take stays in the browser.</p>
    </div></body></html>`)
  await p.waitForTimeout(1500)
  await p.screenshot({ path: `${out}screenshot-2-popup.png` })
  await p.close()
}
await b.close()
