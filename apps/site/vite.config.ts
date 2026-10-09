import { resolve } from 'node:path'
import { defineConfig, type Plugin } from 'vite'

// The site, served at https://fuselane.app (Cloudflare Pages) and, for apps
// already installed, at arshpunisher.github.io/fuselane (GitHub Pages, which
// keeps the update feed and the sign-in check). Paths are relative so the
// same build works in both places; every page names fuselane.app as canonical.
const SITE = 'https://fuselane.app'

// Pages that search engines should list, with how often they change.
const LISTED: [path: string, freq: string, priority: string, name: string][] = [
  ['/', 'weekly', '1.0', 'Fuselane'],
  ['/download/', 'weekly', '0.9', 'Download'],
  ['/faq/', 'monthly', '0.7', 'Questions'],
  ['/support/', 'monthly', '0.5', 'Support'],
]

const text = (html: string) =>
  html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&#39;|&rsquo;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/\s+/g, ' ')
    .trim()

const ld = (data: object) =>
  `<script type="application/ld+json">${JSON.stringify(data).replace(/</g, '\\u003c')}</script>`

function sitemap() {
  const today = new Date().toISOString().slice(0, 10)
  const urls = LISTED.map(
    ([p, freq, priority]) =>
      `  <url>\n    <loc>${SITE}${p}</loc>\n    <lastmod>${today}</lastmod>\n    <changefreq>${freq}</changefreq>\n    <priority>${priority}</priority>\n  </url>`,
  ).join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`
}

/** Structured data, the sitemap and Cloudflare's headers, made from the pages themselves. */
function seo(): Plugin {
  return {
    name: 'fuselane-seo',
    configureServer(server) {
      server.middlewares.use('/sitemap.xml', (_req, res) => {
        res.setHeader('Content-Type', 'application/xml')
        res.end(sitemap())
      })
    },
    transformIndexHtml: {
      order: 'post',
      handler(html, ctx) {
        const path = ctx.path.replace(/index\.html$/, '')
        const page = LISTED.find(([p]) => p === path)
        const extra: string[] = []
        if (path === '/')
          extra.push(
            ld({
              '@context': 'https://schema.org',
              '@type': 'WebSite',
              name: 'Fuselane',
              url: `${SITE}/`,
              publisher: {
                '@type': 'Organization',
                name: 'Fuselane',
                url: `${SITE}/`,
                logo: `${SITE}/icon-512.png`,
                sameAs: ['https://github.com/ArshPunisher/fuselane'],
              },
            }),
          )
        if (page && path !== '/')
          extra.push(
            ld({
              '@context': 'https://schema.org',
              '@type': 'BreadcrumbList',
              itemListElement: [
                { '@type': 'ListItem', position: 1, name: 'Fuselane', item: `${SITE}/` },
                { '@type': 'ListItem', position: 2, name: page[3], item: `${SITE}${path}` },
              ],
            }),
          )
        if (path === '/faq/') {
          // Every <details> on the page is one question and its answer.
          const questions = [
            ...html.matchAll(
              /<details[^>]*>\s*<summary>([\s\S]*?)<\/summary>([\s\S]*?)<\/details>/g,
            ),
          ].map(([, q = '', a = '']) => ({
            '@type': 'Question',
            name: text(q),
            acceptedAnswer: { '@type': 'Answer', text: text(a) },
          }))
          extra.push(
            ld({ '@context': 'https://schema.org', '@type': 'FAQPage', mainEntity: questions }),
          )
        }
        return extra.length ? html.replace('</head>', `${extra.join('\n')}\n</head>`) : html
      },
    },
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: 'sitemap.xml', source: sitemap() })
      // Cloudflare Pages reads these; GitHub Pages ignores them.
      this.emitFile({
        type: 'asset',
        fileName: '_headers',
        source: [
          '/*',
          '  X-Content-Type-Options: nosniff',
          '  Referrer-Policy: strict-origin-when-cross-origin',
          '  Permissions-Policy: camera=(), microphone=(), geolocation=()',
          '  Strict-Transport-Security: max-age=63072000; includeSubDomains; preload',
          '/assets/*',
          '  Cache-Control: public, max-age=31536000, immutable',
          '/s/*',
          '  Referrer-Policy: no-referrer',
          '  X-Robots-Tag: noindex',
          '',
        ].join('\n'),
      })
    },
  }
}

export default defineConfig({
  base: './',
  plugins: [seo()],
  build: {
    target: 'es2022',
    rollupOptions: {
      input: {
        home: resolve(import.meta.dirname, 'index.html'),
        download: resolve(import.meta.dirname, 'download/index.html'),
        faq: resolve(import.meta.dirname, 'faq/index.html'),
        support: resolve(import.meta.dirname, 'support/index.html'),
        send: resolve(import.meta.dirname, 's/index.html'),
        notFound: resolve(import.meta.dirname, '404.html'),
      },
    },
  },
  server: { port: 5195, strictPort: true, fs: { allow: ['../..'] } },
})
