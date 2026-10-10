import { useEffect, useRef } from 'react'
import { bytes } from '../lib/format'
import { t } from '../lib/i18n'
import type { TorrentFileView } from '../lib/types'

/** The torrent's files with a checkbox each; folders are shown dimmed before the name. */
export function FilePicker({
  files,
  chosen,
  onChange,
  idPrefix,
}: {
  files: TorrentFileView[]
  chosen: Set<number>
  onChange(next: Set<number>): void
  idPrefix: string
}) {
  const all = useRef<HTMLInputElement>(null)
  const some = chosen.size > 0 && chosen.size < files.length
  useEffect(() => {
    if (all.current) all.current.indeterminate = some
  }, [some])
  const size = files.filter((f) => chosen.has(f.index)).reduce((a, f) => a + f.size, 0)
  return (
    <fieldset className="pick">
      <legend className="sr-only">{t('Files to download')}</legend>
      <label className="pick-all">
        <input
          ref={all}
          type="checkbox"
          checked={chosen.size === files.length}
          onChange={(e) =>
            onChange(e.target.checked ? new Set(files.map((f) => f.index)) : new Set())
          }
        />
        <span>{t('All files')}</span>
        <span className="pick-sum num" aria-live="polite">
          {t('{chosen} of {total}, {size}', {
            chosen: chosen.size,
            total: files.length,
            size: bytes(size),
          })}
        </span>
      </label>
      <ul className="pick-list">
        {files.map((f) => {
          const cut = f.path.lastIndexOf('/')
          const id = `${idPrefix}-${f.index}`
          return (
            <li key={f.index}>
              <label className="pick-row" htmlFor={id}>
                <input
                  id={id}
                  type="checkbox"
                  checked={chosen.has(f.index)}
                  onChange={(e) => {
                    const next = new Set(chosen)
                    if (e.target.checked) next.add(f.index)
                    else next.delete(f.index)
                    onChange(next)
                  }}
                />
                <span className="pick-path" title={f.path} translate="no">
                  {cut >= 0 && <span className="pick-dir">{f.path.slice(0, cut + 1)}</span>}
                  {f.path.slice(cut + 1)}
                </span>
                <span className="pick-size num">{bytes(f.size)}</span>
              </label>
            </li>
          )
        })}
      </ul>
    </fieldset>
  )
}
