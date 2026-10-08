import { useEffect, useRef, useState } from 'react'
import { X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { PreviewView, UiError } from '../lib/types'
import { bytes } from '../lib/format'

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
  const draft = useApp((s) => s.draft)
  const [preview, setPreview] = useState<
    | { state: 'idle' }
    | { state: 'loading' }
    | { state: 'ok'; data: PreviewView }
    | { state: 'error'; message: string }
  >({ state: 'idle' })

  // Look the link up shortly after typing stops; stale answers are dropped.
  useEffect(() => {
    if (!open || !backend || !looksLikeLink(url)) {
      setPreview({ state: 'idle' })
      return
    }
    let live = true
    setPreview({ state: 'loading' })
    const t = setTimeout(() => {
      backend
        .preview(url.trim())
        .then((data) => live && setPreview({ state: 'ok', data }))
        .catch((e) => live && setPreview({ state: 'error', message: toUiError(e).message }))
    }, 450)
    return () => {
      live = false
      clearTimeout(t)
    }
  }, [url, open, backend])

  useEffect(() => {
    const d = dialog.current
    if (!d) return
    if (open && !d.open) {
      setError(null)
      setBusy(false)
      d.showModal()
      // showModal focuses the first control (Close); the link is what people came for.
      linkInput.current?.focus()
      if (draft) {
        setUrl(draft)
        return
      }
      // Offer a link already on the clipboard (only when the user opened the dialog).
      navigator.clipboard
        ?.readText?.()
        .then((t) => {
          if (looksLikeLink(t)) setUrl((u) => u || t.trim())
        })
        .catch(() => {})
    }
    if (!open && d.open) d.close()
  }, [open, draft])

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
      onClose={() => {
        // A late close event must not shut a dialog that was just reopened.
        if (!dialog.current?.open) setAdding(false)
      }}
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
            <p
              id="nd-url-help"
              className="field-help"
              aria-live="polite"
              data-preview={preview.state}
            >
              {preview.state === 'loading' && 'Looking up the file…'}
              {preview.state === 'ok' && (
                <>
                  <span className="preview-name" translate="no">
                    {preview.data.filename}
                  </span>
                  {preview.data.total !== null
                    ? `, ${bytes(preview.data.total)}`
                    : ', size unknown'}
                  .{' '}
                  {preview.data.splittable
                    ? 'Splits across all your networks.'
                    : "This server won't split the file, so one network will carry it."}
                </>
              )}
              {preview.state === 'error' && preview.message}
              {preview.state === 'idle' && 'Fuselane uses every network that can reach the server.'}
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
