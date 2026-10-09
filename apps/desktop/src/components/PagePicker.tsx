import { useMemo } from 'react'
import { fileType, TYPE_LABEL, type FileType } from '../lib/categories'
import type { PageFiles } from '../lib/types'

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
      const t = fileType(f.name)
      m.set(t, [...(m.get(t) ?? []), f.url])
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
      <div className="page-types" role="group" aria-label="Pick by type">
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
          All <span className="num">{page.files.length}</span>
        </button>
        {[...byType].map(([t, urls]) => {
          const on = urls.every((u) => chosen.has(u))
          return (
            <button
              key={t}
              type="button"
              className="filter"
              aria-pressed={on}
              onClick={() => toggle(urls, !on)}
            >
              {TYPE_LABEL[t]} <span className="num">{urls.length}</span>
            </button>
          )
        })}
      </div>
      <ul className="page-files" aria-label="Files on the page">
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
              <span className="muted page-file-type">{TYPE_LABEL[fileType(f.name)]}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  )
}
