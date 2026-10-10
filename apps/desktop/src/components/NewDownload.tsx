import { useEffect, useRef, useState } from 'react'
import { ArrowLeft, FileArrowUp, Magnet, Plus, Warning, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import { isSendLink } from '../lib/sendLink'
import type {
  HaveView,
  ListingView,
  MediaInfo,
  PageFiles,
  PreviewView,
  UiError,
} from '../lib/types'
import { FilePicker } from './FilePicker'
import { PagePicker } from './PagePicker'
import { REVEAL_LABEL } from './TransferDetail'
import { bytes, clockTime, nextAt, when } from '../lib/format'
import { intlLocale, t, tn, tr } from '../lib/i18n'

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

/** A magnet with a real info hash, shown as a card instead of a long string (B8.3). */
export function magnetCard(s: string): { name: string; hash: string } | null {
  const link = s.trim()
  if (!isMagnet(link) || /\s/.test(link)) return null
  const m = /[?&]xt=urn:bt(?:ih|mh):([0-9a-z]{32,68})/i.exec(link)
  if (!m || !m[1]) return null
  const dn = /[?&]dn=([^&]*)/i.exec(link)?.[1]
  let name = ''
  try {
    name = dn ? decodeURIComponent(dn.replace(/\+/g, ' ')).trim() : ''
  } catch {
    name = dn ?? ''
  }
  const hash = m[1].slice(0, 8).toLowerCase()
  return { name: name || t('Torrent {hash}', { hash }), hash }
}

/** A long single link split so the end (the file name) always shows. */
export function middleCut(s: string): { head: string; tail: string } | null {
  const link = s.trim()
  if (link.length < 48 || !looksLikeLink(link)) return null
  const slash = link.lastIndexOf('/', link.length - 2)
  const tail = slash > 8 && link.length - slash < 60 ? link.slice(slash) : link.slice(-28)
  return { head: link.slice(0, link.length - tail.length), tail }
}

/** A dropped or opened file path (not a link). */
function isTorrentPath(s: string): boolean {
  return /\.torrent$/i.test(s.trim()) && !looksLikeLink(s) && !isMagnet(s)
}

/** A draft from the extension asking for a page's video (api_bridge.rs). */
const VIDEO_DRAFT = 'fuselane-video:'

/** The folder part of a path, for "It's still in ~/Downloads". */
function folderOf(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return i > 0 ? path.slice(0, i) : path
}

/** A Metalink lists files and their mirrors (8.3): Download adds those files. */
function isMetalink(url: string): boolean {
  return /^https?:\/\/\S+\.(meta4|metalink)(\?\S*)?$/i.test(url.trim())
}

export function NewDownload() {
  const open = useApp((s) => s.adding)
  const setAdding = useApp((s) => s.setAdding)
  const backend = useApp((s) => s.backend)
  const info = useApp((s) => s.info)
  const select = useApp((s) => s.select)
  const selectTorrent = useApp((s) => s.selectTorrent)
  const [listing, setListing] = useState<ListingView | null>(null)
  // Find files on a page (B9.3): the page's files, and which are picked.
  const [page, setPage] = useState<PageFiles | null>(null)
  const [picked, setPicked] = useState<Set<string>>(new Set())
  const [pageBusy, setPageBusy] = useState(false)
  // Video pages (B10.4): the qualities yt-dlp found, and the one picked.
  const [media, setMedia] = useState<MediaInfo | null>(null)
  const [quality, setQuality] = useState('')
  const [mediaBusy, setMediaBusy] = useState(false)
  const [chosen, setChosen] = useState<Set<number>>(new Set())
  const [finding, setFinding] = useState(false)
  // Bumped on cancel or close, so a late answer for an abandoned lookup is ignored.
  const lookup = useRef(0)
  const dialog = useRef<HTMLDialogElement>(null)
  const linkInput = useRef<HTMLTextAreaElement>(null)
  const dirInput = useRef<HTMLInputElement>(null)
  const [url, setUrl] = useState('')
  const [focused, setFocused] = useState(false)
  // "Change" on a magnet card brings the text field back until it loses focus.
  const [editing, setEditing] = useState(false)
  const [dir, setDir] = useState('')
  const [more, setMore] = useState(false)
  const [name, setName] = useState('')
  const [sha256, setSha256] = useState('')
  // Start at a set time (B8.5): off, or the next time the clock shows `startTime`.
  const [startLater, setStartLater] = useState(false)
  const [startTime, setStartTime] = useState('02:00')
  // A file of this name is already in the folder (B8.6): keep both or replace.
  const [taken, setTaken] = useState(false)
  const [replace, setReplace] = useState(false)
  // Other links to the same file (B8.9).
  const [mirrors, setMirrors] = useState<string[]>([])
  // Several links: kept together as a group unless unticked (B9.2).
  const [together, setTogether] = useState(true)
  const [groupDraft, setGroupDraft] = useState('')
  const nameRule = useApp((s) => s.automation?.settings.nameTaken ?? 'ask')
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
    if (
      !open ||
      !backend ||
      !looksLikeLink(url) ||
      listing ||
      batchInfo(url).count > 1 ||
      isMetalink(url)
    ) {
      setPreview({ state: 'idle' })
      return
    }
    let live = true
    setPreview({ state: 'loading' })
    const timer = setTimeout(() => {
      backend
        .preview(url.trim())
        .then((data) => live && setPreview({ state: 'ok', data }))
        .catch((e) => live && setPreview({ state: 'error', message: toUiError(e).message }))
    }, 450)
    return () => {
      live = false
      clearTimeout(timer)
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
          .then((text) => {
            if (looksLikeLink(text) || isMagnet(text)) setUrl((u) => u || text.trim())
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

  // Already downloaded (B9.8): the same file finished before and is still on disk.
  const act = useApp((s) => s.act)
  const [have, setHave] = useState<HaveView | null>(null)
  const previewed = preview.state === 'ok' ? preview.data : null
  useEffect(() => {
    if (!open || !backend || !previewed) {
      setHave(null)
      return
    }
    let live = true
    backend
      .alreadyHave(previewed.filename, previewed.total)
      .then((v) => live && setHave(v))
      .catch(() => live && setHave(null))
    return () => {
      live = false
    }
  }, [open, backend, previewed])

  // Ask whether the name is taken once the name is known (Settings: Ask).
  const wantedName = (more && name.trim()) || (preview.state === 'ok' ? preview.data.filename : '')
  useEffect(() => {
    if (!open || !backend || nameRule !== 'ask' || !wantedName) {
      setTaken(false)
      return
    }
    let live = true
    const timer = setTimeout(() => {
      backend
        .nameTaken(dir.trim() || null, wantedName)
        .then((v) => live && setTaken(v))
        .catch(() => live && setTaken(false))
    }, 200)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [open, backend, nameRule, wantedName, dir])

  // Every hand-off (paste, drop, the OS opening a file or magnet) applies, even
  // when the dialog is already open with something else.
  useEffect(() => {
    if (!open) return
    if (draftTorrent) {
      setListing(null)
      void inspect((b) => b.inspectTorrentBytes(draftTorrent, dir.trim() || null))
    } else if (draft.startsWith(VIDEO_DRAFT)) {
      // The extension's "Get the video": straight to the qualities.
      const link = draft.slice(VIDEO_DRAFT.length)
      setListing(null)
      setUrl(link)
      void getVideo(link)
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

  /** Looks a video page up and shows its qualities (B10.4). */
  async function getVideo(link: string) {
    if (!backend) return
    setMediaBusy(true)
    setError(null)
    try {
      const m = await backend.mediaInfo(link)
      setQuality(m.options[0]?.id ?? '')
      setMedia(m)
    } catch (err) {
      setError(toUiError(err))
    } finally {
      setMediaBusy(false)
    }
  }

  function reset() {
    setMedia(null)
    setPage(null)
    setPicked(new Set())
    setUrl('')
    setDir('')
    setName('')
    setSha256('')
    setStartLater(false)
    setReplace(false)
    setMirrors([])
    setMore(false)
    setSkipped([])
  }

  // "Download later" adds paused; a duplicate's "Download again" keeps that choice.
  const later = useRef(false)
  async function submit(e: React.FormEvent | null, allowDuplicate = false, wait?: boolean) {
    e?.preventDefault()
    if (wait !== undefined) later.current = wait
    if (!backend || busy || finding) return
    if (isMagnet(url)) {
      await inspect((b) => b.inspectMagnet(url.trim(), dir.trim() || null))
      return
    }
    // A Fuse Send link isn't a download: receive it on the Send page.
    if (isSendLink(url)) {
      reset()
      useApp.getState().openReceive(url.trim())
      return
    }
    setBusy(true)
    setError(null)
    setSkipped([])
    try {
      if (batch || metalink) {
        const r = metalink
          ? await backend.addMetalink(url.trim(), dir.trim() || null, later.current)
          : await backend.addBatch(
              url,
              dir.trim() || null,
              later.current,
              together ? groupDraft : null,
            )
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
        later: later.current,
        startAt: scheduled,
        replace: taken ? replace : null,
        mirrors: more ? mirrors.map((m) => m.trim()).filter(Boolean) : [],
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
  const metalink = !batch && isMetalink(url)

  const urlError =
    error &&
    (error.code === 'bad-link' || error.code === 'not-a-magnet' || error.code === 'too-many')
      ? error
      : null
  const dirError = error && error.code === 'folder-missing' ? error : null
  const nameError = error && error.code === 'bad-name' ? error : null
  const shaError = error && error.code === 'bad-checksum' ? error : null
  const mirrorError = error && error.code === 'bad-mirror' ? error : null
  const duplicate = error && error.code === 'duplicate' ? error : null
  const otherError =
    error && !urlError && !dirError && !nameError && !shaError && !mirrorError && !duplicate
      ? error
      : null
  const multi = batch || url.includes('\n')
  const scheduled = more && startLater && !batch && !isMagnet(url) ? nextAt(startTime) : null
  const badTime = more && startLater && scheduled === null
  const card = !editing && !urlError ? magnetCard(url) : null
  const cut = !multi ? middleCut(url) : null

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
      {media ? (
        <form
          onSubmit={async (e) => {
            e.preventDefault()
            if (!backend || busy || !quality) return
            setBusy(true)
            setError(null)
            try {
              const ids = await backend.mediaAdd(url.trim(), quality, dir.trim() || null)
              reset()
              setAdding(false)
              if (ids[0] !== undefined) select(ids[0])
            } catch (err) {
              setError(toUiError(err))
            } finally {
              setBusy(false)
            }
          }}
          noValidate
        >
          <header className="dialog-head">
            <h2 id="nd-title">{t('Get the video')}</h2>
            <button
              type="button"
              className="icon-btn"
              aria-label={t('Close')}
              onClick={() => setAdding(false)}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="pick-head">
            <p className="pick-name" translate="no" title={media.title}>
              {media.title}
            </p>
            <p className="field-help page-help">
              {media.duration
                ? t('{site}, {duration}. Downloads over every network at once.', {
                    site: media.site,
                    duration: `${Math.floor(media.duration / 60)}:${String(Math.round(media.duration % 60)).padStart(2, '0')}`,
                  })
                : t('{site}. Downloads over every network at once.', { site: media.site })}
            </p>
          </div>
          <fieldset className="quality-list">
            <legend className="sr-only">{t('Quality')}</legend>
            {media.options.map((o) => (
              <label key={o.id} className="quality">
                <input
                  type="radio"
                  name="quality"
                  value={o.id}
                  checked={quality === o.id}
                  onChange={() => setQuality(o.id)}
                />
                <span className="quality-label">{o.label}</span>
                <span className="muted num">{o.detail}</span>
              </label>
            ))}
          </fieldset>
          {media.hdNeedsFfmpeg && (
            <p className="field-help">
              {t(
                'Higher qualities need the free ffmpeg tool to join video and sound. Install it and look the page up again.',
              )}
            </p>
          )}
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
                setMedia(null)
                setError(null)
              }}
            >
              <ArrowLeft size={16} aria-hidden /> {t('Back')}
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy || !quality}>
              {busy ? t('Adding…') : t('Download')}
            </button>
          </footer>
        </form>
      ) : page ? (
        <form
          onSubmit={async (e) => {
            e.preventDefault()
            if (!backend || busy || picked.size === 0) return
            setBusy(true)
            setError(null)
            try {
              const r = await backend.addBatch(
                [...picked].join('\n'),
                dir.trim() || null,
                false,
                picked.size > 1 ? (page.title ?? '') : null,
              )
              reset()
              setAdding(false)
              if (r.added[0] !== undefined) select(r.added[0])
            } catch (err) {
              setError(toUiError(err))
            } finally {
              setBusy(false)
            }
          }}
          noValidate
        >
          <header className="dialog-head">
            <h2 id="nd-title">{t('Files on the page')}</h2>
            <button
              type="button"
              className="icon-btn"
              aria-label={t('Close')}
              onClick={() => setAdding(false)}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="pick-head">
            <p className="pick-name" translate="no" title={url}>
              {page.title ?? url}
            </p>
            <p className="field-help page-help">
              {tn(
                page.files.length,
                '{n} file. Pick what to download; more than one stay together as a group.',
                '{n} files. Pick what to download; more than one stay together as a group.',
              )}
            </p>
          </div>
          <PagePicker page={page} chosen={picked} onChange={setPicked} />
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
                setPage(null)
                setError(null)
              }}
            >
              <ArrowLeft size={16} aria-hidden /> {t('Back')}
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy || picked.size === 0}>
              {busy
                ? t('Adding…')
                : picked.size
                  ? t('Download {n}', { n: picked.size })
                  : t('Download')}
            </button>
          </footer>
        </form>
      ) : listing ? (
        <form onSubmit={startTorrent} noValidate>
          <header className="dialog-head">
            <h2 id="nd-title">{t('Choose files')}</h2>
            <button
              type="button"
              className="icon-btn"
              aria-label={t('Close')}
              onClick={() => setAdding(false)}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="pick-head">
            <p className="pick-name" translate="no" title={listing.name}>
              {listing.name}
            </p>
            <p className="field-help" title={listing.folder}>
              {tr('Saves to {folder}', { folder: <span translate="no">{listing.folder}</span> })}
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
              <ArrowLeft size={16} aria-hidden /> {t('Back')}
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy || chosen.size === 0}>
              {busy ? t('Starting…') : t('Download')}
            </button>
          </footer>
        </form>
      ) : (
        <form onSubmit={submit} noValidate>
          <header className="dialog-head">
            <h2 id="nd-title">{t('New download')}</h2>
            <button
              type="button"
              className="icon-btn"
              aria-label={t('Close')}
              onClick={() => setAdding(false)}
            >
              <X size={18} aria-hidden />
            </button>
          </header>
          <div className="field">
            <label htmlFor="nd-url">{t(batch ? 'Links' : 'Link')}</label>
            {card ? (
              <div className="link-card">
                <span className="link-card-ic" aria-hidden>
                  <Magnet size={18} weight="fill" />
                </span>
                <span className="link-card-text">
                  <span className="link-card-name" translate="no" title={card.name}>
                    {card.name}
                  </span>
                  <span className="link-card-meta num">
                    {t('Magnet link, {hash}', { hash: card.hash })}
                  </span>
                </span>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  aria-label={t('Change the link')}
                  onClick={() => {
                    setEditing(true)
                    requestAnimationFrame(() => linkInput.current?.focus())
                  }}
                >
                  {t('Change')}
                </button>
              </div>
            ) : (
              <div className="link-line-wrap" data-cut={cut && !focused ? '' : undefined}>
                <textarea
                  id="nd-url"
                  name="url"
                  inputMode="url"
                  autoComplete="off"
                  spellCheck={false}
                  ref={linkInput}
                  value={url}
                  // One line for one link (no wrapping, no scrollbar); a list of
                  // links grows to show them, one per line.
                  rows={multi ? Math.min(8, Math.max(3, url.split('\n').length)) : 1}
                  wrap={multi ? 'soft' : 'off'}
                  className="link-input"
                  data-multi={multi ? '' : undefined}
                  placeholder={t('https://example.com/file.iso or magnet:?…')}
                  aria-invalid={urlError ? true : undefined}
                  aria-describedby={urlError ? 'nd-url-err' : 'nd-url-help'}
                  onFocus={() => setFocused(true)}
                  onBlur={() => {
                    setFocused(false)
                    setEditing(false)
                  }}
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
                {cut && !focused && (
                  // Long links are cut in the middle so the file name stays in view.
                  <span className="link-cut" aria-hidden>
                    <span className="head">{cut.head}</span>
                    <span className="tail">{cut.tail}</span>
                  </span>
                )}
              </div>
            )}
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
                {preview.state === 'loading' && t('Looking up the file…')}
                {preview.state === 'ok' && (
                  <>
                    {preview.data.total !== null
                      ? tr('{name}, {size}.', {
                          name: (
                            <span className="preview-name" translate="no">
                              {preview.data.filename}
                            </span>
                          ),
                          size: bytes(preview.data.total),
                        })
                      : tr('{name}, size unknown.', {
                          name: (
                            <span className="preview-name" translate="no">
                              {preview.data.filename}
                            </span>
                          ),
                        })}{' '}
                    {preview.data.splittable
                      ? t('Splits across all your networks.')
                      : t("This server won't split the file, so one network will carry it.")}
                  </>
                )}
                {preview.state === 'error' && preview.message}
                {batch &&
                  (pattern
                    ? tn(
                        count,
                        '{n} link with a pattern. Each [01-20] becomes one download per number.',
                        '{n} links with a pattern. Each [01-20] becomes one download per number.',
                      )
                    : t(
                        '{n} links. Each becomes its own download; ones already in your list are skipped.',
                        { n: count },
                      ))}
                {!batch &&
                  preview.state === 'idle' &&
                  (metalink
                    ? t(
                        'A Metalink: Fuselane downloads the files it lists, each from all its mirrors, and checks them.',
                      )
                    : finding
                      ? t(
                          "Finding the torrent's files. This can take a minute when few people share it.",
                        )
                      : isMagnet(url)
                        ? t('A magnet link. Next you pick which of its files to download.')
                        : t('Fuselane uses every network that can reach the server.'))}
              </p>
            )}
            {!batch && !metalink && !isMagnet(url) && /^https?:\/\/\S+$/i.test(url.trim()) && (
              <button
                type="button"
                className="link-btn find-files"
                disabled={pageBusy}
                onClick={async () => {
                  if (!backend) return
                  setPageBusy(true)
                  setError(null)
                  try {
                    const p = await backend.filesOnPage(url.trim())
                    setPicked(new Set())
                    setPage(p)
                  } catch (err) {
                    setError(toUiError(err))
                  } finally {
                    setPageBusy(false)
                  }
                }}
              >
                {pageBusy ? t('Reading the page…') : t('Find files on this page')}
              </button>
            )}
            {!batch && !metalink && !isMagnet(url) && /^https?:\/\/\S+$/i.test(url.trim()) && (
              <button
                type="button"
                className="link-btn find-files"
                disabled={mediaBusy}
                onClick={() => void getVideo(url.trim())}
              >
                {mediaBusy ? t('Looking for the video…') : t('Get the video from this page')}
              </button>
            )}
          </div>
          {batch && count > 1 && (
            <div className="group-choice">
              <label className="check">
                <input
                  type="checkbox"
                  checked={together}
                  onChange={(e) => setTogether(e.target.checked)}
                />
                <span>
                  {t('Keep them together as a group')}
                  <span className="field-help">
                    {t('One row in your list with one progress, and one notice when all are done.')}
                  </span>
                </span>
              </label>
              {together && (
                <input
                  aria-label={t('Group name')}
                  className="group-name"
                  maxLength={80}
                  placeholder={t('Name (optional)')}
                  value={groupDraft}
                  onChange={(e) => setGroupDraft(e.target.value)}
                />
              )}
            </div>
          )}
          {taken && !batch && !isMagnet(url) && (
            <div className="dup" role="group" aria-labelledby="nd-dup">
              <div className="dup-row">
                <Warning size={16} weight="fill" aria-hidden />
                <span id="nd-dup">
                  {have
                    ? t(
                        "You already downloaded this ({size}, {date}). It's still in this folder.",
                        {
                          size: bytes(have.size),
                          date: new Date(have.finishedAt * 1000).toLocaleDateString(intlLocale()),
                        },
                      )
                    : t('A file named {name} is already in this folder.', { name: wantedName })}
                </span>
              </div>
              <div className="dup-row">
                <div className="segmented" role="radiogroup" aria-labelledby="nd-dup">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!replace}
                    onClick={() => setReplace(false)}
                  >
                    {t('Keep both')}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={replace}
                    onClick={() => setReplace(true)}
                  >
                    {t('Replace')}
                  </button>
                </div>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    reset()
                    setAdding(false)
                  }}
                >
                  {t("Don't download")}
                </button>
                {have && (
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={() => void act((b) => b.reveal(have.id))}
                  >
                    {t(REVEAL_LABEL)}
                  </button>
                )}
              </div>
            </div>
          )}
          <div className="field">
            <label htmlFor="nd-dir">{t('Save to')}</label>
            <div className="field-row">
              <input
                id="nd-dir"
                name="dir"
                autoComplete="off"
                spellCheck={false}
                value={dir}
                ref={dirInput}
                placeholder={`${info?.defaultDir ?? t('Downloads')}…`}
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
                {t('Choose…')}
              </button>
            </div>
            {dirError ? (
              <p id="nd-dir-err" className="field-error" aria-live="polite">
                {dirError.message} {dirError.hint}
              </p>
            ) : (
              <p id="nd-dir-help" className="field-help">
                {info?.defaultDir
                  ? t('Leave empty to use {folder}.', { folder: info.defaultDir })
                  : t('Leave empty to use your Downloads folder.')}
              </p>
            )}
          </div>
          {!batch && !isMagnet(url) && (
            <details
              className="more-options"
              open={more}
              onToggle={(e) => setMore(e.currentTarget.open)}
            >
              <summary>{t('More options')}</summary>
              <div className="field">
                <label htmlFor="nd-name">{t('Save as')}</label>
                <input
                  id="nd-name"
                  name="filename"
                  autoComplete="off"
                  spellCheck={false}
                  value={name}
                  placeholder={
                    preview.state === 'ok' ? `${preview.data.filename}…` : t("The server's name…")
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
                    : t('Leave empty to keep the name the server gives it.')}
                </p>
              </div>
              <div className="field">
                <label htmlFor="nd-sha">{t('SHA-256 to check')}</label>
                <input
                  id="nd-sha"
                  name="sha256"
                  autoComplete="off"
                  spellCheck={false}
                  className="num"
                  value={sha256}
                  placeholder={t('64 letters and digits from the download page…')}
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
                    : t(
                        "Fuselane checks the finished file and won't save it under its name if it differs.",
                      )}
                </p>
              </div>
              <div className="field">
                <span className="label" id="nd-mirrors-label">
                  {t('Mirrors')}
                </span>
                <div className="mirrors" role="group" aria-labelledby="nd-mirrors-label">
                  {mirrors.map((m, i) => (
                    <div className="mirror" key={i}>
                      <input
                        autoComplete="off"
                        spellCheck={false}
                        inputMode="url"
                        aria-label={t('Mirror {n}', { n: i + 1 })}
                        // i18n-ignore: an example address
                        placeholder="https://mirror.example.org/same-file.iso…"
                        value={m}
                        aria-invalid={mirrorError ? true : undefined}
                        onChange={(e) => {
                          const next = [...mirrors]
                          next[i] = e.target.value
                          setMirrors(next)
                          if (mirrorError) setError(null)
                        }}
                      />
                      <button
                        type="button"
                        className="icon-btn"
                        aria-label={t('Remove mirror {n}', { n: i + 1 })}
                        onClick={() => setMirrors(mirrors.filter((_, j) => j !== i))}
                      >
                        <X size={16} aria-hidden />
                      </button>
                    </div>
                  ))}
                  {mirrors.length < 8 && (
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm add-mirror"
                      onClick={() => setMirrors([...mirrors, ''])}
                    >
                      <Plus size={16} aria-hidden /> {t('Add a mirror')}
                    </button>
                  )}
                </div>
                <p className={mirrorError ? 'field-error' : 'field-help'}>
                  {mirrorError
                    ? `${mirrorError.message} ${mirrorError.hint ?? ''}`
                    : t(
                        'The same file on other servers. Each is checked first, then every network fetches from all of them.',
                      )}
                </p>
              </div>
              <div className="field">
                <span className="label" id="nd-start-label">
                  {t('Start')}
                </span>
                <div className="inline-row">
                  <div className="segmented" role="radiogroup" aria-labelledby="nd-start-label">
                    <button
                      type="button"
                      role="radio"
                      aria-checked={!startLater}
                      onClick={() => setStartLater(false)}
                    >
                      {t('Now')}
                    </button>
                    <button
                      type="button"
                      role="radio"
                      aria-checked={startLater}
                      onClick={() => setStartLater(true)}
                    >
                      {t('At a time')}
                    </button>
                  </div>
                  {startLater && (
                    <>
                      <input
                        type="time"
                        className="num time-in"
                        aria-label={t('Start time')}
                        value={startTime}
                        aria-invalid={badTime || undefined}
                        onChange={(e) => setStartTime(e.target.value)}
                      />
                      {scheduled !== null && <span className="muted">{when(scheduled)}</span>}
                    </>
                  )}
                </div>
                <p className="field-help">
                  {startLater
                    ? t('It waits in your list and starts by itself, even with the window closed.')
                    : t('Starts as soon as there is room in the queue.')}
                </p>
              </div>
            </details>
          )}
          {have && !duplicate && !(taken && !batch) && (
            <div className="inline-note have-note" role="status">
              <p>
                {tr(
                  "You already downloaded this: {name}, {size}, on {date}. It's still in {folder}.",
                  {
                    name: <strong translate="no">{have.name}</strong>,
                    size: bytes(have.size),
                    date: new Date(have.finishedAt * 1000).toLocaleDateString(intlLocale()),
                    folder: <span translate="no">{folderOf(have.path)}</span>,
                  },
                )}
              </p>
              <button
                type="button"
                className="btn"
                onClick={() => void act((b) => b.reveal(have.id))}
              >
                {t(REVEAL_LABEL)}
              </button>
            </div>
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
                {t('Download again')}
              </button>
            </div>
          )}
          {skipped.length > 0 && (
            <div className="field-error" role="alert">
              <p>
                {tn(
                  skipped.length,
                  "One link was not added. The rest started. They're left in the box above:",
                  "{n} links were not added. The rest started. They're left in the box above:",
                )}
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
                {skipped.length > 5 && <li>{t('and {n} more.', { n: skipped.length - 5 })}</li>}
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
              <FileArrowUp size={16} aria-hidden /> {t('Open .torrent…')}
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
              {t('Cancel')}
            </button>
            {!isMagnet(url) && scheduled === null && (
              <button
                type="button"
                className="btn"
                onClick={() => void submit(null, false, true)}
                disabled={busy || finding}
                title={t('Add it to the list without starting it')}
              >
                {t('Download later')}
              </button>
            )}
            <button
              type="submit"
              className="btn btn-primary"
              disabled={busy || finding || badTime}
              onClick={() => (later.current = false)}
            >
              {finding
                ? t('Finding files…')
                : busy
                  ? t('Starting…')
                  : isMagnet(url)
                    ? t('Next')
                    : batch
                      ? count > 1 && !pattern
                        ? t('Download {n}', { n: count })
                        : t('Download all')
                      : scheduled !== null
                        ? t('Download at {time}', { time: clockTime(scheduled) })
                        : t('Download')}
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}
