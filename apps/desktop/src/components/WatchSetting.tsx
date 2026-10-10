import { useEffect, useState } from 'react'
import { CheckCircle, FolderOpen, WarningCircle } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { UiError, WatchView } from '../lib/types'
import { t } from '../lib/i18n'

/**
 * Watch folder (B10.9): .torrent files, Metalinks and lists of links put in
 * a folder are added by themselves. Media tools hand work over this way.
 */
export function WatchSetting() {
  const backend = useApp((s) => s.backend)
  const [view, setView] = useState<WatchView | null>(null)
  const [error, setError] = useState<UiError | null>(null)
  useEffect(() => {
    if (!backend) return
    const load = () => void backend.watchState().then(setView)
    load()
    // The latest files handled change while the page is open.
    const timer = setInterval(load, 4000)
    return () => clearInterval(timer)
  }, [backend])

  async function set(on: boolean, path: string) {
    if (!backend) return
    setError(null)
    try {
      setView(await backend.watchSet(on, path))
    } catch (e) {
      setError(toUiError(e))
    }
  }

  async function choose(): Promise<string | null> {
    const picked = await backend?.pickFolder().catch(() => null)
    return picked ?? null
  }

  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div>
          <p className="setting-name">{t('Add files from a folder')}</p>
          <p className="muted">
            {t(
              'Put a .torrent file, a Metalink or a .txt list of links in this folder and it starts by itself; the file is then renamed to end in .added. Media tools that save .torrent files to a folder work with it.',
            )}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          className="switch"
          aria-label={t('Add files from a folder')}
          aria-checked={view?.on === true}
          disabled={view === null}
          onClick={async () => {
            if (!view) return
            if (view.on) return void set(false, '')
            const path = view.path || (await choose())
            if (path) void set(true, path)
          }}
        />
      </div>
      {error && (
        <p className="field-error" role="alert">
          {error.message} {error.hint}
        </p>
      )}
      {view?.on && (
        <div className="watch">
          <div className="watch-folder">
            <span className="num" translate="no">
              {view.path}
            </span>
            <button
              type="button"
              className="btn btn-sm"
              onClick={async () => {
                const path = await choose()
                if (path) void set(true, path)
              }}
            >
              <FolderOpen size={16} aria-hidden /> {t('Change…')}
            </button>
          </div>
          {view.problem && (
            <p className="field-error" role="alert">
              {view.problem}
            </p>
          )}
          {view.recent.length > 0 ? (
            <ul className="watch-recent" aria-label={t('Files taken from the folder')}>
              {view.recent.slice(0, 5).map((h, i) => (
                <li key={`${h.name}-${i}`} data-ok={h.ok}>
                  {h.ok ? (
                    <CheckCircle size={16} weight="fill" aria-label={t('Added')} />
                  ) : (
                    <WarningCircle size={16} weight="fill" aria-label={t('Not added')} />
                  )}
                  <span className="watch-name" translate="no">
                    {h.name}
                  </span>
                  <span className="muted">{h.note}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="field-help">{t('Watching. Nothing has been put in it yet.')}</p>
          )}
        </div>
      )}
    </div>
  )
}
