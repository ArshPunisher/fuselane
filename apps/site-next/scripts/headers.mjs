// Writes out/_headers for Cloudflare Pages after `next build`.
//
// The plain site's Content Security Policy allowed no inline script at all.
// Next.js puts a few inline scripts in every page (the React payload), so
// instead of loosening the policy to 'unsafe-inline', each page gets a policy
// that allows exactly its own inline scripts by SHA-256 hash. Anything else
// injected into a page still can't run.
//
// Cloudflare joins headers from every matching rule, so the catch-all rule's
// policy (sized for the 404 page, which unknown paths get) is detached with
// "! Content-Security-Policy" before each page sets its own.
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { join, relative, sep } from 'node:path'

const OUT = 'out'
const base = (scriptHashes) =>
  [
    "default-src 'self'",
    `script-src 'self' ${scriptHashes.join(' ')}`.trim(),
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    "font-src 'self'",
    "connect-src 'self' https://api.github.com",
    "manifest-src 'self'",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'none'",
    'upgrade-insecure-requests',
  ].join('; ')

function walk(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f)
    return statSync(p).isDirectory() ? walk(p) : p.endsWith('.html') ? [p] : []
  })
}

function hashes(html) {
  const out = new Set()
  for (const m of html.matchAll(/<script(?![^>]*\bsrc=)([^>]*)>([\s\S]*?)<\/script>/g)) {
    if (/type="application\/ld\+json"/.test(m[1])) continue // data, not script
    out.add(`'sha256-${createHash('sha256').update(m[2], 'utf8').digest('base64')}'`)
  }
  return [...out]
}

const pages = walk(OUT).map((file) => {
  const rel = relative(OUT, file).split(sep).join('/')
  const route =
    rel === 'index.html'
      ? '/'
      : rel.endsWith('/index.html')
        ? `/${rel.slice(0, -'index.html'.length)}`
        : `/${rel}`
  return { route, rel, csp: base(hashes(readFileSync(file, 'utf8'))) }
})

const notFound = pages.find((p) => p.rel === '404.html')
const lines = [
  '/*',
  `  Content-Security-Policy: ${notFound ? notFound.csp : base([])}`,
  '  X-Content-Type-Options: nosniff',
  '  Referrer-Policy: strict-origin-when-cross-origin',
  '  Permissions-Policy: camera=(), microphone=(), geolocation=()',
  '  Strict-Transport-Security: max-age=63072000; includeSubDomains; preload',
  '/_next/static/*',
  '  Cache-Control: public, max-age=31536000, immutable',
]
for (const p of pages) {
  if (p.rel === '404.html') continue
  for (const path of p.route === '/' ? ['/', '/index.html'] : [p.route, `${p.route}index.html`]) {
    lines.push(path, '  ! Content-Security-Policy', `  Content-Security-Policy: ${p.csp}`)
    if (p.route === '/s/') lines.push('  Referrer-Policy: no-referrer', '  X-Robots-Tag: noindex')
  }
}
// The share page keeps no-referrer for every link token under it.
lines.push(
  '/s/*',
  '  ! Referrer-Policy',
  '  Referrer-Policy: no-referrer',
  '  X-Robots-Tag: noindex',
  '',
)

const longest = Math.max(...lines.map((l) => l.length))
if (longest > 2000) throw new Error(`_headers line too long for Cloudflare (${longest} > 2000)`)
writeFileSync(join(OUT, '_headers'), lines.join('\n'))
console.log(`_headers: ${pages.length} pages, longest line ${longest} chars`)
