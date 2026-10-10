import { useMemo } from 'react'
import { fileType, typeLabel, type FileType } from '../lib/categories'
import type { PageFiles } from '../lib/types'
import { t, tr } from '../lib/i18n'

/**
 * The files a web page links to (B9.3), picked by type or one by one. Nothing
 * is downloaded until "Download" is pressed in the dialog.
 */
export function PagePicker({
  page,
  chosen,
  onChange,
}: {
  page: PageFiles
  chosen: Set<string>
  onChange: (next: Set<string>) => void
}) {
  const byType = useMemo(() => {
    const m = new Map<FileType, string[]>()
    for (const f of page.files) {
      const type = fileType(f.name)
      m.set(type, [...(m.get(type) ?? []), f.url])
    }
    return m
  }, [page])
  const allChosen = chosen.size === page.files.length
  const toggle = (urls: string[], on: boolean) => {
    const next = new Set(chosen)
    for (const u of urls) {
      if (on) next.add(u)
      else next.delete(u)
    }
    onChange(next)
  }
  return (
    <div className="page-picker">
      <div className="page-types" role="group" aria-label={t('Pick by type')}>
        <button
          type="button"
          className="filter"
          aria-pressed={allChosen}
          onClick={() =>
            toggle(
              page.files.map((f) => f.url),
              !allChosen,
            )
          }
        >
          {tr('All {n}', { n: <span className="num">{page.files.length}</span> })}
        </button>
        {[...byType].map(([type, urls]) => {
          const on = urls.every((u) => chosen.has(u))
          return (
            <button
              key={type}
              type="button"
              className="filter"
              aria-pressed={on}
              onClick={() => toggle(urls, !on)}
            >
              {typeLabel(type)} <span className="num">{urls.length}</span>
            </button>
          )
        })}
      </div>
      <ul className="page-files" aria-label={t('Files on the page')}>
        {page.files.map((f) => (
          <li key={f.url}>
            <label className="check">
              <input
                type="checkbox"
                checked={chosen.has(f.url)}
                onChange={(e) => toggle([f.url], e.target.checked)}
              />
              <span className="page-file-name" translate="no" title={f.url}>
                {f.name}
              </span>
              <span className="muted page-file-type">{typeLabel(fileType(f.name))}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  )
}
