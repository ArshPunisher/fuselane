import { useEffect, useRef, useState } from 'react'
import { ArrowClockwise, Rss, Trash, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { FeedView, UiError } from '../lib/types'

const EVERY: { mins: number; label: string }[] = [
  { mins: 15, label: 'Every 15 minutes' },
  { mins: 60, label: 'Every hour' },
  { mins: 360, label: 'Every 6 hours' },
  { mins: 1440, label: 'Once a day' },
]

const STATE_WORD: Record<FeedView['recent'][number]['state'], string> = {
  added: 'Downloading',
  filtered: 'Skipped by your words',
  'no-file': 'No file in it',
  torrent: 'Torrent',
  failed: "Couldn't add",
}

function ago(unix: number | null): string {
  if (unix === null) return 'Not checked yet'
  const mins = Math.max(0, Math.round((Date.now() / 1000 - unix) / 60))
  if (mins < 1) return 'Checked just now'
  if (mins < 60) return `Checked ${mins} min ago`
  const hours = Math.round(mins / 60)
  return hours < 48 ? `Checked ${hours} h ago` : `Checked ${Math.round(hours / 24)} days ago`
}

function FeedCard({
  feed,
  onChange,
  onLeave,
}: {
  feed: FeedView
  onChange: (f: FeedView[]) => void
  /** Closes the Feeds window (before New download opens over it). */
  onLeave: () => void
}) {
  const backend = useApp((s) => s.backend)
  const setAdding = useApp((s) => s.setAdding)
  const [editing, setEditing] = useState(false)
  const [include, setInclude] = useState(feed.include)
  const [exclude, setExclude] = useState(feed.exclude)
  const [every, setEvery] = useState(feed.every)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function run(f: () => Promise<FeedView[]>) {
    setBusy(true)
    setError(null)
    try {
      onChange(await f())
      return true
    } catch (e) {
      const u = toUiError(e)
      setError([u.message, u.hint].filter(Boolean).join(' '))
      return false
    } finally {
      setBusy(false)
    }
  }

  const every_label = EVERY.find((e) => e.mins === feed.every)?.label.toLowerCase() ?? ''
  return (
    <li className="feed">
      <div className="feed-head">
        <span className="feed-ic" aria-hidden>
          <Rss size={16} />
        </span>
        <div className="feed-text">
          <p className="setting-name" translate="no">
            {feed.title}
          </p>
          <p className="muted feed-meta">
            {ago(feed.lastCheck)}, {every_label}. {feed.added} downloaded
            {feed.include ? `, only with “${feed.include}”` : ''}
            {feed.exclude ? `, skipping “${feed.exclude}”` : ''}.
          </p>
        </div>
        <div className="feed-actions">
          <button
            type="button"
            className="icon-btn"
            aria-label={`Check ${feed.title} now`}
            disabled={busy}
            onClick={() => void run(() => backend!.feedsCheck(feed.id))}
          >
            <ArrowClockwise size={16} aria-hidden />
          </button>
          <button
            type="button"
            className="btn btn-sm"
            aria-expanded={editing}
            onClick={() => setEditing(!editing)}
          >
            Filters
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={`Stop following ${feed.title}`}
            disabled={busy}
            onClick={() => void run(() => backend!.feedsRemove(feed.id))}
          >
            <Trash size={16} aria-hidden />
          </button>
        </div>
      </div>
      {feed.problem && (
        <p className="field-error" role="alert">
          Last check: {feed.problem}
        </p>
      )}
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      {editing && (
        <form
          className="feed-filters"
          onSubmit={(e) => {
            e.preventDefault()
            void run(() => backend!.feedsUpdate(feed.id, include, exclude, every)).then(
              (ok) => ok && setEditing(false),
            )
          }}
        >
          <FilterFields
            idBase={`feed-${feed.id}`}
            include={include}
            exclude={exclude}
            every={every}
            setInclude={setInclude}
            setExclude={setExclude}
            setEvery={setEvery}
          />
          <button type="submit" className="btn" disabled={busy}>
            Save
          </button>
        </form>
      )}
      {feed.recent.length > 0 && (
        <ul className="feed-recent" aria-label={`Latest in ${feed.title}`}>
          {feed.recent.slice(0, 4).map((r, i) => (
            <li key={`${r.url}-${i}`}>
              <span className="feed-item" translate="no">
                {r.title || r.url}
              </span>
              <span className="chip" data-state={r.state}>
                {r.note ?? STATE_WORD[r.state]}
              </span>
              {r.state === 'torrent' && (
                <button
                  type="button"
                  className="link-btn"
                  onClick={() => {
                    onLeave()
                    setAdding(true, r.url)
                  }}
                >
                  Open
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </li>
  )
}

function FilterFields(p: {
  idBase: string
  include: string
  exclude: string
  every: number
  setInclude: (v: string) => void
  setExclude: (v: string) => void
  setEvery: (v: number) => void
}) {
  return (
    <>
      <div className="field">
        <label htmlFor={`${p.idBase}-inc`}>Only titles with</label>
        <input
          id={`${p.idBase}-inc`}
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder="e.g. 1080p"
          value={p.include}
          onChange={(e) => p.setInclude(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor={`${p.idBase}-exc`}>Skip titles with</label>
        <input
          id={`${p.idBase}-exc`}
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder="e.g. trailer"
          value={p.exclude}
          onChange={(e) => p.setExclude(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor={`${p.idBase}-every`}>Check</label>
        <select
          id={`${p.idBase}-every`}
          value={p.every}
          onChange={(e) => p.setEvery(Number(e.target.value))}
        >
          {EVERY.map((e) => (
            <option key={e.mins} value={e.mins}>
              {e.label}
            </option>
          ))}
        </select>
      </div>
    </>
  )
}

/**
 * Feeds (B10.8): follow a podcast, a project's releases or any RSS or Atom
 * feed; new files in it download by themselves.
 */
export function FeedsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null)
  const backend = useApp((s) => s.backend)
  const [feeds, setFeeds] = useState<FeedView[] | null>(null)
  const [url, setUrl] = useState('')
  const [include, setInclude] = useState('')
  const [exclude, setExclude] = useState('')
  const [every, setEvery] = useState(60)
  const [latest, setLatest] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<UiError | null>(null)
  const [status, setStatus] = useState('')

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setError(null)
      setStatus('')
      // A feed handed over by the extension: ready to follow with one click.
      const draft = useApp.getState().feedDraft
      if (draft) {
        setUrl(draft)
        useApp.setState({ feedDraft: null })
      }
      d.showModal()
      void backend?.feedsList().then(setFeeds)
    }
    if (!open && d.open) d.close()
  }, [open, backend])

  async function follow() {
    if (!backend || busy) return
    setBusy(true)
    setError(null)
    setStatus('')
    try {
      setFeeds(await backend.feedsAdd(url, include, exclude, every, latest))
      setStatus(
        latest
          ? 'Following. The latest file is downloading; new ones follow by themselves.'
          : 'Following. New files download by themselves.',
      )
      setUrl('')
      setInclude('')
      setExclude('')
    } catch (e) {
      setError(toUiError(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <dialog
      ref={ref}
      className="dialog feeds-dialog"
      aria-labelledby="feeds-title"
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      {open && (
        <div className="feeds">
          <header className="dialog-head">
            <h2 id="feeds-title">Feeds</h2>
            <button type="button" className="icon-btn" aria-label="Close" onClick={onClose}>
              <X size={16} aria-hidden />
            </button>
          </header>
          <p className="dialog-text">
            Follow a podcast, a project&apos;s releases or any RSS or Atom feed. New files in it
            download by themselves, over every network. Torrents wait for you to open them.
          </p>
          <form
            className="feed-add"
            noValidate
            onSubmit={(e) => {
              e.preventDefault()
              void follow()
            }}
          >
            <div className="field feed-url">
              <label htmlFor="feed-url">Feed address</label>
              <input
                id="feed-url"
                type="url"
                inputMode="url"
                autoComplete="off"
                spellCheck={false}
                placeholder="https://example.com/feed.xml"
                value={url}
                aria-invalid={error ? true : undefined}
                aria-describedby={error ? 'feed-url-err' : undefined}
                onChange={(e) => setUrl(e.target.value)}
              />
              {error && (
                <p id="feed-url-err" className="field-error">
                  {error.message} {error.hint}
                </p>
              )}
            </div>
            <FilterFields
              idBase="feed-new"
              include={include}
              exclude={exclude}
              every={every}
              setInclude={setInclude}
              setExclude={setExclude}
              setEvery={setEvery}
            />
            <label className="check feed-latest">
              <input
                type="checkbox"
                checked={latest}
                onChange={(e) => setLatest(e.target.checked)}
              />
              <span>Download the latest one now</span>
            </label>
            <button type="submit" className="btn btn-primary" disabled={busy || !url.trim()}>
              <Rss size={16} aria-hidden /> {busy ? 'Reading the feed…' : 'Follow'}
            </button>
          </form>
          <p className="muted" role="status">
            {status}
          </p>
          {feeds && feeds.length > 0 && (
            <ul className="feed-list" aria-label="Feeds you follow">
              {feeds.map((f) => (
                <FeedCard key={f.id} feed={f} onChange={setFeeds} onLeave={onClose} />
              ))}
            </ul>
          )}
          {feeds && feeds.length === 0 && (
            <p className="muted">No feeds yet. Paste one above to start.</p>
          )}
        </div>
      )}
    </dialog>
  )
}
