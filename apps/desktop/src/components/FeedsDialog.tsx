import { useEffect, useRef, useState } from 'react'
import { ArrowClockwise, Rss, Trash, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { FeedView, UiError } from '../lib/types'
import { mark, t } from '../lib/i18n'

const EVERY: { mins: number; label: string }[] = [
  { mins: 15, label: mark('Every 15 minutes') },
  { mins: 60, label: mark('Every hour') },
  { mins: 360, label: mark('Every 6 hours') },
  { mins: 1440, label: mark('Once a day') },
]

const STATE_WORD: Record<FeedView['recent'][number]['state'], string> = {
  added: mark('Downloading'),
  filtered: mark('Skipped by your words'),
  'no-file': mark('No file in it'),
  torrent: mark('Torrent'),
  'torrent-started': mark('Torrent started'),
  failed: mark("Couldn't add"),
}

/** A magnet or a .torrent file: something New download can open. */
function isTorrentLink(url: string): boolean {
  return url.startsWith('magnet:') || url.endsWith('.torrent')
}

function ago(unix: number | null): string {
  if (unix === null) return t('Not checked yet')
  const mins = Math.max(0, Math.round((Date.now() / 1000 - unix) / 60))
  if (mins < 1) return t('Checked just now')
  if (mins < 60) return t('Checked {n} min ago', { n: mins })
  const hours = Math.round(mins / 60)
  return hours < 48
    ? t('Checked {n} h ago', { n: hours })
    : t('Checked {n} days ago', { n: Math.round(hours / 24) })
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
  const [startTorrents, setStartTorrents] = useState(feed.startTorrents)
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

  const everyLabel = EVERY.find((e) => e.mins === feed.every)?.label
  // "every hour" in a sentence; Hindi has no capitals, so this leaves it as it is.
  const everyText = everyLabel ? t(everyLabel).toLowerCase() : ''
  const details = [
    t('{n} downloaded', { n: feed.added }),
    feed.include ? t('only with “{words}”', { words: feed.include }) : '',
    feed.exclude ? t('skipping “{words}”', { words: feed.exclude }) : '',
  ]
    .filter(Boolean)
    .join(', ')
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
            {t('{checked}, {every}. {details}.', {
              checked: ago(feed.lastCheck),
              every: everyText,
              details,
            })}
            {feed.startTorrents && <> {t('Torrents start by themselves.')}</>}
          </p>
        </div>
        <div className="feed-actions">
          <button
            type="button"
            className="icon-btn"
            aria-label={t('Check {name} now', { name: feed.title })}
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
            {t('Filters')}
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={t('Stop following {name}', { name: feed.title })}
            disabled={busy}
            onClick={() => void run(() => backend!.feedsRemove(feed.id))}
          >
            <Trash size={16} aria-hidden />
          </button>
        </div>
      </div>
      {feed.problem && (
        <p className="field-error" role="alert">
          {t('Last check: {problem}', { problem: feed.problem })}
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
            void run(() =>
              backend!.feedsUpdate(feed.id, include, exclude, every, startTorrents),
            ).then((ok) => ok && setEditing(false))
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
          <TorrentsCheck checked={startTorrents} onChange={setStartTorrents} />
          <button type="submit" className="btn" disabled={busy}>
            {t('Save')}
          </button>
        </form>
      )}
      {feed.recent.length > 0 && (
        <ul className="feed-recent" aria-label={t('Latest in {name}', { name: feed.title })}>
          {feed.recent.slice(0, 4).map((r, i) => (
            <li key={`${r.url}-${i}`}>
              <span className="feed-item" translate="no">
                {r.title || r.url}
              </span>
              <span className="chip" data-state={r.state}>
                {r.note ?? t(STATE_WORD[r.state])}
              </span>
              {(r.state === 'torrent' || (r.state === 'failed' && isTorrentLink(r.url))) && (
                <button
                  type="button"
                  className="link-btn"
                  onClick={() => {
                    onLeave()
                    setAdding(true, r.url)
                  }}
                >
                  {t('Open')}
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
        <label htmlFor={`${p.idBase}-inc`}>{t('Only titles with')}</label>
        <input
          id={`${p.idBase}-inc`}
          name="include"
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder={t('e.g. 1080p')}
          value={p.include}
          onChange={(e) => p.setInclude(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor={`${p.idBase}-exc`}>{t('Skip titles with')}</label>
        <input
          id={`${p.idBase}-exc`}
          name="exclude"
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder={t('e.g. trailer')}
          value={p.exclude}
          onChange={(e) => p.setExclude(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor={`${p.idBase}-every`}>{t('Check')}</label>
        <select
          id={`${p.idBase}-every`}
          name="every"
          value={p.every}
          onChange={(e) => p.setEvery(Number(e.target.value))}
        >
          {EVERY.map((e) => (
            <option key={e.mins} value={e.mins}>
              {t(e.label)}
            </option>
          ))}
        </select>
      </div>
    </>
  )
}

/** Off unless the person lets a feed start torrents (all their files) by itself. */
function TorrentsCheck({
  checked,
  onChange,
}: {
  checked: boolean
  onChange: (on: boolean) => void
}) {
  return (
    <label className="check feed-torrents">
      <input
        type="checkbox"
        name="startTorrents"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{t('Start torrents by themselves')}</span>
    </label>
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
  const [startTorrents, setStartTorrents] = useState(false)
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
      setFeeds(await backend.feedsAdd(url, include, exclude, every, latest, startTorrents))
      // Kept in English and translated when shown, so it follows a language change.
      setStatus(
        latest
          ? mark('Following. The latest file is downloading; new ones follow by themselves.')
          : mark('Following. New files download by themselves.'),
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
            <h2 id="feeds-title">{t('Feeds')}</h2>
            <button type="button" className="icon-btn" aria-label={t('Close')} onClick={onClose}>
              <X size={16} aria-hidden />
            </button>
          </header>
          <p className="dialog-text">
            {t(
              "Follow a podcast, a project's releases or any RSS or Atom feed. New files in it download by themselves, over every network. Torrents wait for you to open them, unless you let them start by themselves.",
            )}
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
              <label htmlFor="feed-url">{t('Feed address')}</label>
              <input
                id="feed-url"
                name="feed"
                type="url"
                inputMode="url"
                autoComplete="off"
                spellCheck={false}
                // i18n-ignore: an example address
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
            <div className="feed-checks">
              <label className="check">
                <input
                  type="checkbox"
                  name="latest"
                  checked={latest}
                  onChange={(e) => setLatest(e.target.checked)}
                />
                <span>{t('Download the latest one now')}</span>
              </label>
              <TorrentsCheck checked={startTorrents} onChange={setStartTorrents} />
            </div>
            <button type="submit" className="btn btn-primary" disabled={busy || !url.trim()}>
              <Rss size={16} aria-hidden /> {busy ? t('Reading the feed…') : t('Follow')}
            </button>
          </form>
          <p className="muted" role="status">
            {status && t(status)}
          </p>
          {feeds && feeds.length > 0 && (
            <ul className="feed-list" aria-label={t('Feeds you follow')}>
              {feeds.map((f) => (
                <FeedCard key={f.id} feed={f} onChange={setFeeds} onLeave={onClose} />
              ))}
            </ul>
          )}
          {feeds && feeds.length === 0 && (
            <p className="muted">{t('No feeds yet. Paste one above to start.')}</p>
          )}
        </div>
      )}
    </dialog>
  )
}
