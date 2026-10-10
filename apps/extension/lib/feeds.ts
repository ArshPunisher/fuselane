/** A feed a page advertises (`<link rel="alternate" type="application/rss+xml">`). */
export interface FoundFeed {
  href: string
  title: string
}

/** Most feeds offered for one page. */
export const MAX_FEEDS = 3

/**
 * Runs in the page (injected with activeTab, so it must stand alone): the
 * feeds the page links to in its head.
 */
export function findFeeds(): FoundFeed[] {
  const out: FoundFeed[] = []
  document.querySelectorAll('link[rel~="alternate"][href]').forEach((el) => {
    const l = el as HTMLLinkElement
    const type = (l.getAttribute('type') || '').toLowerCase()
    if (/(rss|atom)\+xml/.test(type)) out.push({ href: l.href, title: l.title || '' })
  })
  return out
}

/** Feeds worth offering: absolute http(s), each once, at most MAX_FEEDS. */
export function feedList(found: FoundFeed[], pageUrl: string): FoundFeed[] {
  const seen = new Set<string>()
  const out: FoundFeed[] = []
  for (const f of found) {
    let url: URL
    try {
      url = new URL(f.href, pageUrl)
    } catch {
      continue
    }
    if (url.protocol !== 'https:' && url.protocol !== 'http:') continue
    if (seen.has(url.href)) continue
    seen.add(url.href)
    out.push({ href: url.href, title: f.title.trim().slice(0, 80) })
    if (out.length >= MAX_FEEDS) break
  }
  return out
}
