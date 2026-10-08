import { useEffect, useRef, useState } from 'react'
import { ArrowLeft, FileArrowUp, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { ListingView, PreviewView, UiError } from '../lib/types'
import { FilePicker } from './FilePicker'
import { bytes } from '../lib/format'

/** Quick local check so obvious mistakes show before a round trip; the backend decides. */
function looksLikeLink(s: string): boolean {
  return /^https?:\/\/\S+$/i.test(s.trim())
}

function isMagnet(s: string): boolean {
  return /^magnet:\?/i.test(s.trim())
}

/** Every http(s) link in pasted text, and whether any is a pattern like part[01-10].zip. */
function batchInfo(s: string): { count: number; pattern: boolean } {
  const links = s.match(/https?:\/\/[^\s"<>]+/gi) ?? []
  return {
    count: new Set(links).size,
    pattern: links.some((l) => /\[(\d+-\d+|[a-z]-[a-z]|[A-Z]-[A-Z])\]/.test(l)),
  }
}

/** A dropped or opened file path (not a link). */
function isTorrentPath(s: string): boolean {
  return /\.torrent$/i.test(s.trim()) && !looksLikeLink(s) && !isMagnet(s)
}

export function NewDownload() {
  const open = useApp((s) => s.adding)
  const setAdding = useApp((s) => s.setAdding)
  const backend = useApp((s) => s.backend)
  const info = useApp((s) => s.info)
  const select = useApp((s) => s.select)
  const selectTorrent = useApp((s) => s.selectTorrent)
  const [listing, setListing] = useState<ListingView | null>(null)
  const [chosen, setChosen] = useState<Set<number>>(new Set())
  const [finding, setFinding] = useState(false)
  // Bumped on cancel or close, so a late answer for an abandoned lookup is ignored.
  const lookup = useRef(0)
  const dialog = useRef<HTMLDialogElement>(null)
  const linkInput = useRef<HTMLTextAreaElement>(null)
  const dirInput = useRef<HTMLInputElement>(null)
  const [url, setUrl] = useState('')
  const [dir, setDir] = useState('')
  const [more, setMore] = useState(false)
  const [name, setName] = useState('')
  const [sha256, setSha256] = useState('')
  /** What a batch add skipped, shown until the dialog closes. */
  const [skipped, setSkipped] = useState<{ url: string; reason: string }[]>([])
  const [error, setError] = useState<UiError | null>(null)
  const [busy, setBusy] = useState(false)
  const draft = useApp((s) => s.draft)
  const draftTorrent = useApp((s) => s.draftTorrent)
  const draftSeq = useApp((s) => s.draftSeq)
  const [preview, setPreview] = useState<
    | { state: 'idle' }
    | { state: 'loading' }
    | { state: 'ok'; data: PreviewView }
    | { state: 'error'; message: string }
  >({ state: 'idle' })

  // Look the link up shortly after typing stops; stale answers are dropped.
  useEffect(() => {
    if (!open || !backend || !looksLikeLink(url) || listing || batchInfo(url).count > 1) {
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
  }, [url, open, backend, listing])

  useEffect(() => {
    const d = dialog.current
    if (!d) return
    if (open && !d.open) {
      setError(null)
      setBusy(false)
      d.showModal()
      // showModal focuses the first control (Close); the link is what people came for.
      linkInput.current?.focus()
      if (!draft && !draftTorrent) {
        // Offer a link already on the clipboard (only when the user opened the dialog).
        navigator.clipboard
          ?.readText?.()
          .then((t) => {
            if (looksLikeLink(t) || isMagnet(t)) setUrl((u) => u || t.trim())
          })
          .catch(() => {})
      }
    }
    if (!open && d.open) d.close()
    if (!open) {
      lookup.current++
      setListing(null)
      setFinding(false)
      setSkipped([])
    }
  }, [open, draft, draftTorrent])

  // Every hand-off (paste, drop, the OS opening a file or magnet) applies, even
  // when the dialog is already open with something else.
  useEffect(() => {
    if (!open) return
    if (draftTorrent) {
      setListing(null)
      void inspect((b) => b.inspectTorrentBytes(draftTorrent, dir.trim() || null))
    } else if (draft && isTorrentPath(draft)) {
      setListing(null)
      void inspect((b) => b.inspectTorrentFile(draft, dir.trim() || null))
    } else if (draft) {
      setListing(null)
      setError(null)
      setUrl(draft)
    }
    // dir is read, not watched: changing the folder must not re-run a hand-off.
  }, [open, draftSeq])

  /** Reads a torrent's files, then shows the picker. */
  async function inspect(f: (b: NonNullable<typeof backend>) => Promise<ListingView>) {
    if (!backend) return
    const mine = ++lookup.current
    setFinding(true)
    setError(null)
    try {
      const l = await f(backend)
      if (mine !== lookup.current) return
      setListing(l)
      setChosen(new Set(l.files.map((x) => x.index)))
    } catch (err) {
      if (mine === lookup.current) setError(toUiError(err))
    } finally {
      if (mine === lookup.current) setFinding(false)
    }
  }

  async function openTorrentFile() {
    const path = await backend?.pickTorrent().catch(() => null)
    if (path) await inspect((b) => b.inspectTorrentFile(path, dir.trim() || null))
  }

  async function startTorrent(e: React.FormEvent) {
    e.preventDefault()
    if (!backend || busy || !listing) return
    setBusy(true)
    setError(null)
    try {
      const id = await backend.addTorrent(listing.token, [...chosen])
      setUrl('')
      setDir('')
      setListing(null)
      setAdding(false)
      selectTorrent(id)
    } catch (err) {
      setError(toUiError(err))
    } finally {
      setBusy(false)
    }
  }

  function reset() {
    setUrl('')
    setDir('')
    setName('')
    setSha256('')
    setMore(false)
    setSkipped([])
  }

  async function submit(e: React.FormEvent | null, allowDuplicate = false) {
    e?.preventDefault()
    if (!backend || busy || finding) return
    if (isMagnet(url)) {
      await inspect((b) => b.inspectMagnet(url.trim(), dir.trim() || null))
      return
    }
    setBusy(true)
    setError(null)
    setSkipped([])
    try {
      if (batch) {
        const r = await backend.addBatch(url, dir.trim() || null)
        if (r.skipped.length === 0) {
          reset()
          setAdding(false)
          if (r.added[0] !== undefined) select(r.added[0])
        } else {
          // Keep the dialog open with what was skipped and why.
          setUrl(r.skipped.map((s) => s.url).join('\n'))
          setSkipped(r.skipped)
        }
        return
      }
      const id = await backend.add(url, dir.trim() || null, {
        name: more ? name.trim() || null : null,
        sha256: more ? sha256.trim() || null : null,
        allowDuplicate,
      })
      reset()
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

  const { count, pattern } = batchInfo(url)
  const batch = !isMagnet(url) && (count > 1 || pattern)
  const urlError =
    error &&
    (error.code === 'bad-link' || error.code === 'not-a-magnet' || error.code === 'too-many')
      ? error
      : null
  const dirError = error && error.code === 'folder-missing' ? error : null
  const nameError = error && error.code === 'bad-name' ? error : null
  const shaError = error && error.code === 'bad-checksum' ? error : null
  const duplicate = error && error.code === 'duplicate' ? error : null
  const otherError =
    error && !urlError && !dirError && !nameError && !shaError && !duplicate ? error : null

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
      {listing ? (
        <form onSubmit={startTorrent} noValidate>
          <header className="dialog-head">
            <h2 id="nd-title">Choose files</h2>
            <button
              type="button"
              className="icon-btn"
              aria-label="Close"
              onClick={() => setAdding(false)}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="pick-head">
            <p className="pick-name" translate="no" title={listing.name}>
              {listing.name}
            </p>
            <p className="field-help" translate="no" title={listing.folder}>
              Saves to {listing.folder}
            </p>
          </div>
          <FilePicker
            files={listing.files}
            chosen={chosen}
            onChange={setChosen}
            idPrefix="nd-file"
          />
          {error && (
            <p className="field-error" role="alert">
              {error.message} {error.hint}
            </p>
          )}
          <footer className="dialog-foot">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => {
                setListing(null)
                setError(null)
              }}
            >
              <ArrowLeft size={16} aria-hidden /> Back
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy || chosen.size === 0}>
              {busy ? 'Starting…' : 'Download'}
            </button>
          </footer>
        </form>
      ) : (
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
            <label htmlFor="nd-url">{batch ? 'Links' : 'Link'}</label>
            <textarea
              id="nd-url"
              name="url"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              ref={linkInput}
              value={url}
              rows={batch ? Math.min(8, Math.max(3, url.split('\n').length)) : 1}
              className="link-input"
              placeholder="https://example.com/file.iso or magnet:?…"
              aria-invalid={urlError ? true : undefined}
              aria-describedby={urlError ? 'nd-url-err' : 'nd-url-help'}
              onChange={(e) => {
                setUrl(e.target.value)
                if (urlError || duplicate) setError(null)
              }}
              onKeyDown={(e) => {
                // Enter starts the download; Shift+Enter adds another line for more links.
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault()
                  e.currentTarget.form?.requestSubmit()
                }
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
                {batch &&
                  (pattern
                    ? `${count} ${count === 1 ? 'link' : 'links'} with a pattern. Each [01-20] becomes one download per number.`
                    : `${count} links. Each becomes its own download; ones already in your list are skipped.`)}
                {!batch &&
                  preview.state === 'idle' &&
                  (finding
                    ? "Finding the torrent's files. This can take a minute when few people share it."
                    : isMagnet(url)
                      ? 'A magnet link. Next you pick which of its files to download.'
                      : 'Fuselane uses every network that can reach the server.')}
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
          {!batch && !isMagnet(url) && (
            <details
              className="more-options"
              open={more}
              onToggle={(e) => setMore(e.currentTarget.open)}
            >
              <summary>More options</summary>
              <div className="field">
                <label htmlFor="nd-name">Save as</label>
                <input
                  id="nd-name"
                  name="filename"
                  autoComplete="off"
                  spellCheck={false}
                  value={name}
                  placeholder={
                    preview.state === 'ok' ? `${preview.data.filename}…` : "The server's name…"
                  }
                  aria-invalid={nameError ? true : undefined}
                  aria-describedby="nd-name-help"
                  onChange={(e) => {
                    setName(e.target.value)
                    if (nameError) setError(null)
                  }}
                />
                <p id="nd-name-help" className={nameError ? 'field-error' : 'field-help'}>
                  {nameError
                    ? `${nameError.message} ${nameError.hint ?? ''}`
                    : 'Leave empty to keep the name the server gives it.'}
                </p>
              </div>
              <div className="field">
                <label htmlFor="nd-sha">SHA-256 to check</label>
                <input
                  id="nd-sha"
                  name="sha256"
                  autoComplete="off"
                  spellCheck={false}
                  className="num"
                  value={sha256}
                  placeholder="64 letters and digits from the download page…"
                  aria-invalid={shaError ? true : undefined}
                  aria-describedby="nd-sha-help"
                  onChange={(e) => {
                    setSha256(e.target.value)
                    if (shaError) setError(null)
                  }}
                />
                <p id="nd-sha-help" className={shaError ? 'field-error' : 'field-help'}>
                  {shaError
                    ? `${shaError.message} ${shaError.hint ?? ''}`
                    : "Fuselane checks the finished file and won't save it under its name if it differs."}
                </p>
              </div>
            </details>
          )}
          {duplicate && (
            <div className="inline-note" role="alert">
              <p>{duplicate.message}</p>
              <button
                type="button"
                className="btn"
                onClick={() => void submit(null, true)}
                disabled={busy}
              >
                Download again
              </button>
            </div>
          )}
          {skipped.length > 0 && (
            <div className="field-error" role="alert">
              <p>
                {skipped.length === 1 ? 'One link was' : `${skipped.length} links were`} not added.
                The rest started. They're left in the box above:
              </p>
              <ul className="skipped">
                {skipped.slice(0, 5).map((s) => (
                  <li key={s.url}>
                    <span className="num" translate="no">
                      {s.url}
                    </span>
                    : {s.reason}
                  </li>
                ))}
                {skipped.length > 5 && <li>and {skipped.length - 5} more.</li>}
              </ul>
            </div>
          )}
          {otherError && (
            <p className="field-error" role="alert">
              {otherError.message} {otherError.hint}
            </p>
          )}
          <footer className="dialog-foot">
            <button
              type="button"
              className="btn btn-ghost foot-start"
              onClick={() => void openTorrentFile()}
              disabled={finding}
            >
              <FileArrowUp size={16} aria-hidden /> Open .torrent…
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => {
                if (finding) {
                  lookup.current++
                  setFinding(false)
                } else setAdding(false)
              }}
            >
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy || finding}>
              {finding
                ? 'Finding files…'
                : busy
                  ? 'Starting…'
                  : isMagnet(url)
                    ? 'Next'
                    : batch
                      ? `Download ${count > 1 && !pattern ? count : 'all'}`
                      : 'Download'}
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}
