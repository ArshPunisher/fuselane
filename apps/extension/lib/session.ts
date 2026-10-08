// Signed-in downloads (BROWSER-EXTENSION.md §4): optional, asked for only when the
// person switches it on. With it, a hand-off carries the site's cookies and the
// browser's User-Agent to the Fuselane app on this computer, and nowhere else.
import { cookieHeader } from '@fuselane/capture'
import type { Session } from './handoff.ts'

export const SESSION_PERMISSIONS: Browser.permissions.Permissions = {
  permissions: ['cookies'],
  origins: ['<all_urls>'],
}

export async function sessionAllowed(): Promise<boolean> {
  try {
    return await browser.permissions.contains(SESSION_PERMISSIONS)
  } catch {
    return false
  }
}

/** The session for `url`, or null when not allowed. Partitioned cookies included. */
export async function getSession(url: string): Promise<Session | null> {
  if (!(await sessionAllowed())) return null
  const cookies = await browser.cookies
    .getAll({ url, partitionKey: {} })
    .catch(() => browser.cookies.getAll({ url }))
  return {
    cookies: cookieHeader(cookies) || null,
    userAgent: navigator.userAgent || null,
  }
}
