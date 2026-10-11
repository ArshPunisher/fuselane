// Clicks on file links go straight to Fuselane, so the browser never starts them
// (REVIEW-2026-10-11 D, like IDM). Registered at run time, only once the person
// allowed Fuselane on all sites. Hold Alt to let the browser have a link.
export default defineContentScript({
  registration: 'runtime',
  matches: ['<all_urls>'],
  runAt: 'document_start',
  main() {
    const FILES = new Set(
      (
        '7z aac ace apk appimage arj avi bin bz2 cab deb dmg exe flac flv gz img iso jar ' +
        'lz lzh m4a m4v mkv mov mp3 mp4 mpeg mpg msi msix ogg ogv pkg rar rpm tar tgz ' +
        'torrent wav webm wma wmv xz zip zst'
      ).split(' '),
    )
    let on = true
    const read = () =>
      browser.storage.local
        .get('rules')
        .then(({ rules }) => {
          on = !(
            rules &&
            typeof rules === 'object' &&
            (rules as { enabled?: boolean }).enabled === false
          )
        })
        .catch(() => {})
    /** This page's copy of the script can still reach the extension. */
    const alive = () => {
      try {
        return Boolean(browser.runtime?.id)
      } catch {
        return false
      }
    }
    void read()
    browser.storage.onChanged.addListener((c) => {
      if (c.rules && alive()) void read()
    })

    const isFile = (a: HTMLAnchorElement): boolean => {
      if (a.hasAttribute('download')) return true
      try {
        const u = new URL(a.href)
        if (u.protocol !== 'https:' && u.protocol !== 'http:') return false
        const last = u.pathname.split('/').pop() ?? ''
        const dot = last.lastIndexOf('.')
        return dot > 0 && FILES.has(last.slice(dot + 1).toLowerCase())
      } catch {
        return false
      }
    }

    document.addEventListener(
      'click',
      (e) => {
        if (!on || e.defaultPrevented || e.button !== 0) return
        if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return
        const a = (e.target as Element | null)?.closest?.('a[href]') as HTMLAnchorElement | null
        if (!a || !isFile(a)) return
        let url: URL
        try {
          url = new URL(a.href)
        } catch {
          return
        }
        if (url.protocol !== 'https:' && url.protocol !== 'http:') return
        // After the extension is reloaded or updated, this copy in an open page
        // can't reach it any more: leave the click to the browser (no error,
        // and the link still works).
        if (!alive()) return
        e.preventDefault()
        e.stopImmediatePropagation()
        browser.runtime
          .sendMessage({
            type: 'grab',
            url: url.href,
            page: location.href,
            name: a.getAttribute('download') || null,
          })
          .catch(() => {
            // Lost between the check and the send: the browser gets it after all.
            location.assign(url.href)
          })
      },
      true,
    )
  },
})
