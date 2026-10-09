// "On this page": the videos, audio and file links a page offers, found when the
// popup opens (activeTab: only the tab the person is looking at, only then).

export interface Found {
  url: string
  /** What the page called it: a link's text, a title, or nothing. */
  label: string
  kind: 'video' | 'audio' | 'link'
}

export interface MediaItem extends Found {
  /** A short name for the list: the file name from the link. */
  name: string
}

/** At most this many are listed; pages with hundreds of links are rare and noisy. */
export const MAX_ITEMS = 40

const FILE_TYPES = new Set(
  (
    'zip rar 7z tar gz tgz bz2 xz zst iso img dmg exe msi msix pkg deb rpm appimage apk ' +
    'mp4 m4v mkv avi mov webm wmv flv mpg mpeg mp3 m4a aac flac wav ogg opus ' +
    'pdf epub torrent bin safetensors gguf'
  ).split(' '),
)

function extension(u: URL): string {
  const last = u.pathname.split('/').pop() ?? ''
  const dot = last.lastIndexOf('.')
  return dot > 0 ? last.slice(dot + 1).toLowerCase() : ''
}

function nameOf(u: URL): string {
  const last = u.pathname.split('/').pop() ?? ''
  try {
    return decodeURIComponent(last) || u.hostname
  } catch {
    return last || u.hostname
  }
}

/**
 * Turns what the page script found into the list to show: web links only (a
 * `blob:` stream can't be fetched again), links only when they point at a file,
 * each link once, media first.
 */
export function mediaList(found: Found[], pageUrl: string): MediaItem[] {
  const seen = new Set<string>()
  const out: MediaItem[] = []
  for (const f of found) {
    let u: URL
    try {
      u = new URL(f.url, pageUrl)
    } catch {
      continue
    }
    if (u.protocol !== 'http:' && u.protocol !== 'https:') continue
    if (f.kind === 'link' && !FILE_TYPES.has(extension(u))) continue
    u.hash = ''
    const key = u.href
    if (seen.has(key)) continue
    seen.add(key)
    out.push({ url: key, kind: f.kind, label: f.label.trim().slice(0, 120), name: nameOf(u) })
  }
  const rank = { video: 0, audio: 1, link: 2 } as const
  return out.sort((a, b) => rank[a.kind] - rank[b.kind]).slice(0, MAX_ITEMS)
}

/**
 * Runs inside the page (scripting.executeScript): must not use anything from
 * outside this function.
 */
export function findOnPage(): Found[] {
  const out: Found[] = []
  const add = (url: string | null | undefined, kind: Found['kind'], label: string) => {
    if (url) out.push({ url, kind, label })
  }
  document.querySelectorAll('video, audio').forEach((el) => {
    const m = el as HTMLMediaElement
    const kind = m.tagName === 'VIDEO' ? 'video' : 'audio'
    const label = m.getAttribute('title') || m.getAttribute('aria-label') || document.title
    add(m.currentSrc || m.src, kind, label)
    m.querySelectorAll('source').forEach((s) => add(s.src, kind, label))
  })
  document.querySelectorAll('a[href]').forEach((el) => {
    const a = el as HTMLAnchorElement
    add(a.href, 'link', a.textContent ?? '')
  })
  return out.slice(0, 2000)
}
