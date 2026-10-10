// Fuse Send links: the share page's address, or the app's own fuselane://send/ form.
export const SEND_PAGE = 'https://fuselane.app/s#'
/** Where the page used to be: links made by older versions still open. */
const OLD_PAGES = ['https://arshpunisher.github.io/fuselane/s#']
export const SEND_SCHEME = 'fuselane://send/'

/** A Fuse Send link opens the receive form, not the download dialog. */
export function isSendLink(text: string): boolean {
  const t = text.trim()
  return (
    t.startsWith(SEND_PAGE) ||
    OLD_PAGES.some((p) => t.startsWith(p)) ||
    t.toLowerCase().startsWith(SEND_SCHEME)
  )
}
