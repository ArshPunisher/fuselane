// "Open this page in Fuselane" (the popup's This page tab): hands the tab's link
// to the app, whose New download dialog then offers what the app can get from
// it, as for a pasted link. The extension never looks at what the page is or
// says what it holds: that is the app's business (Chrome Web Store policy, see
// BROWSER-EXTENSION.md §1 and L-127).

import type { AppState } from './status.ts'

/** Only web pages can be handed over; the app refuses anything else too. */
export function canOpenPage(url: string | undefined): url is string {
  if (!url || url.length > 8192) return false
  try {
    const u = new URL(url)
    return u.protocol === 'https:' || u.protocol === 'http:'
  } catch {
    return false
  }
}

/** What the popup does with the app's answer: close, or say what went wrong. */
export type OpenOutcome = { close: true } | { close: false; text: string }

export function openPageOutcome(reply: unknown, state: AppState): OpenOutcome {
  const r = (reply && typeof reply === 'object' ? reply : {}) as {
    type?: unknown
    reason?: unknown
  }
  if (r.type === 'page.open.opened') return { close: true }
  if (state === 'not-installed') {
    return { close: false, text: 'This needs the Fuselane app. Get it, then try again.' }
  }
  // An app from before page.open answers an unknown message with "invalid".
  if (r.type === 'download.declined' && r.reason === 'invalid') {
    return { close: false, text: 'Update Fuselane to open pages from here, then try again.' }
  }
  return { close: false, text: "Fuselane didn't answer. Open it and try again." }
}
