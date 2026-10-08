// The settings page's form ⇄ Rules, kept free of the DOM so it can be tested
// without a browser (test/settings-form.test.ts).
import {
  formatSize,
  parseDomains,
  parseExtensions,
  parseMimeTypes,
  parseSize,
  type ListResult,
  type Rules,
} from '@fuselane/capture'

export interface FormValues {
  enabled: boolean
  minSize: string
  includeDomains: string
  excludeDomains: string
  extensions: string
  mimeTypes: string
}

export type Field = Exclude<keyof FormValues, 'enabled'>

export type FormResult =
  { ok: true; rules: Rules } | { ok: false; errors: Partial<Record<Field, string>> }

export function toForm(r: Rules): FormValues {
  return {
    enabled: r.enabled,
    minSize: formatSize(r.minBytes),
    includeDomains: r.includeDomains.join('\n'),
    excludeDomains: r.excludeDomains.join('\n'),
    extensions: r.extensions.join(' '),
    mimeTypes: r.mimeTypes.join(' '),
  }
}

const quote = (entries: string[]) => entries.map((e) => `“${e}”`).join(', ')

function listError(r: ListResult, what: string, example: string): string | undefined {
  if (r.invalid.length === 0) return undefined
  const one = r.invalid.length === 1
  return `${quote(r.invalid)} ${one ? `isn’t a ${what}` : `aren’t ${what}s`}. Use something like ${example}.`
}

export function readForm(v: FormValues): FormResult {
  const errors: Partial<Record<Field, string>> = {}
  const size = parseSize(v.minSize)
  if (!size.ok) errors.minSize = size.error

  const include = parseDomains(v.includeDomains)
  const exclude = parseDomains(v.excludeDomains)
  const extensions = parseExtensions(v.extensions)
  const mimeTypes = parseMimeTypes(v.mimeTypes)

  const site = (r: ListResult) => listError(r, 'site name', 'example.org')
  if (site(include)) errors.includeDomains = site(include)
  if (site(exclude)) errors.excludeDomains = site(exclude)
  else {
    const both = exclude.values.filter((d) => include.values.includes(d))
    if (both.length > 0)
      errors.excludeDomains = `${quote(both)} ${both.length === 1 ? 'is' : 'are'} in both lists. Remove it from one of them.`
  }
  const ext = listError(extensions, 'file type', 'iso or zip')
  if (ext) errors.extensions = ext
  const mime = listError(mimeTypes, 'MIME type', 'application/zip or video/*')
  if (mime) errors.mimeTypes = mime

  if (!size.ok || Object.keys(errors).length > 0) return { ok: false, errors }
  return {
    ok: true,
    rules: {
      enabled: v.enabled,
      minBytes: size.value,
      includeDomains: include.values,
      excludeDomains: exclude.values,
      extensions: extensions.values,
      mimeTypes: mimeTypes.values,
    },
  }
}
