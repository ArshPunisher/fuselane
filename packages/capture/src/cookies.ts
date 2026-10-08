// The Cookie header the app sends with a captured download (BROWSER-EXTENSION.md §1).

export interface CookieLike {
  name: string
  value: string
}

/** No separators or line breaks that could split the header or add another. */
const SAFE_NAME = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/
const UNSAFE_VALUE = /[;\r\n\0]/

/**
 * `a=b; c=d` from the browser's cookies, in the order given (the browser lists
 * the most specific first). Later duplicates and unsafe names or values are
 * dropped rather than sent.
 */
export function cookieHeader(cookies: CookieLike[]): string {
  const seen = new Set<string>()
  const parts: string[] = []
  for (const c of cookies) {
    if (!SAFE_NAME.test(c.name) || UNSAFE_VALUE.test(c.value) || seen.has(c.name)) continue
    seen.add(c.name)
    parts.push(`${c.name}=${c.value}`)
  }
  return parts.join('; ')
}
