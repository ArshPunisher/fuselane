import { useEffect, useRef, useState } from 'react'
import { X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { UiError } from '../lib/types'

/** Quick local check so obvious mistakes show before a round trip; the backend decides. */
function looksLikeLink(s: string): boolean {
  return /^https?:\/\/\S+$/i.test(s.trim())
}

export function NewDownload() {
  const open = useApp((s) => s.adding)
  const setAdding = useApp((s) => s.setAdding)
  const backend = useApp((s) => s.backend)
  const info = useApp((s) => s.info)
  const select = useApp((s) => s.select)
  const dialog = useRef<HTMLDialogElement>(null)
  const linkInput = useRef<HTMLInputElement>(null)
  const dirInput = useRef<HTMLInputElement>(null)
  const [url, setUrl] = useState('')
  const [dir, setDir] = useState('')
  const [error, setError] = useState<UiError | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    const d = dialog.current
    if (!d) return
    if (open && !d.open) {
      setError(null)
      setBusy(false)
      d.showModal()
      // showModal focuses the first control (Close); the link is what people came for.
      linkInput.current?.focus()
      // Offer a link already on the clipboard (only when the user opened the dialog).
      navigator.clipboard
        ?.readText?.()
        .then((t) => {
          if (looksLikeLink(t)) setUrl((u) => u || t.trim())
        })
        .catch(() => {})
    }
    if (!open && d.open) d.close()
  }, [open])

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!backend || busy) return
    setBusy(true)
    setError(null)
    try {
      const id = await backend.add(url, dir.trim() || null)
      setUrl('')
      setDir('')
      setAdding(false)
      select(id)
    } catch (err) {
      const e = toUiError(err)
      setError(e)
      // Put the cursor where the fix goes.
      ;(e.code === 'folder-missing' ? dirInput : linkInput).current?.focus()
    } finally {
      setBusy(false)
    }
  }

  const urlError = error && error.code === 'bad-link' ? error : null
  const dirError = error && error.code === 'folder-missing' ? error : null
  const otherError = error && !urlError && !dirError ? error : null

  return (
    <dialog
      ref={dialog}
      className="dialog"
      aria-labelledby="nd-title"
      onClose={() => setAdding(false)}
    >
      <form onSubmit={submit} noValidate>
        <header className="dialog-head">
          <h2 id="nd-title">New download</h2>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close"
            onClick={() => setAdding(false)}
          >
            <X size={18} aria-hidden />
          </button>
        </header>
        <div className="field">
          <label htmlFor="nd-url">Link</label>
          <input
            id="nd-url"
            name="url"
            type="url"
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            ref={linkInput}
            value={url}
            placeholder="https://example.com/file.iso…"
            aria-invalid={urlError ? true : undefined}
            aria-describedby={urlError ? 'nd-url-err' : 'nd-url-help'}
            onChange={(e) => {
              setUrl(e.target.value)
              if (urlError) setError(null)
            }}
          />
          {urlError ? (
            <p id="nd-url-err" className="field-error" aria-live="polite">
              {urlError.message} {urlError.hint}
            </p>
          ) : (
            <p id="nd-url-help" className="field-help">
              Fuselane uses every network that can reach the server.
            </p>
          )}
        </div>
        <div className="field">
          <label htmlFor="nd-dir">Save to</label>
          <div className="field-row">
            <input
              id="nd-dir"
              name="dir"
              autoComplete="off"
              spellCheck={false}
              value={dir}
              ref={dirInput}
              placeholder={`${info?.defaultDir ?? 'Downloads'}…`}
              aria-invalid={dirError ? true : undefined}
              aria-describedby={dirError ? 'nd-dir-err' : 'nd-dir-help'}
              onChange={(e) => {
                setDir(e.target.value)
                if (dirError) setError(null)
              }}
            />
            <button
              type="button"
              className="btn btn-ghost field-side"
              onClick={async () => {
                const picked = await backend?.pickFolder().catch(() => null)
                if (picked) {
                  setDir(picked)
                  if (dirError) setError(null)
                }
              }}
            >
              Choose…
            </button>
          </div>
          {dirError ? (
            <p id="nd-dir-err" className="field-error" aria-live="polite">
              {dirError.message} {dirError.hint}
            </p>
          ) : (
            <p id="nd-dir-help" className="field-help">
              Leave empty to use {info?.defaultDir ?? 'your Downloads folder'}.
            </p>
          )}
        </div>
        {otherError && (
          <p className="field-error" role="alert">
            {otherError.message} {otherError.hint}
          </p>
        )}
        <footer className="dialog-foot">
          <button type="button" className="btn btn-ghost" onClick={() => setAdding(false)}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {busy ? 'Starting…' : 'Download'}
          </button>
        </footer>
      </form>
    </dialog>
  )
}
