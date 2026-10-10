import { useCallback, useEffect, useState } from 'react'
import {
  ArrowSquareOut,
  Browser,
  GoogleChromeLogo,
  Plugs,
  PlugsConnected,
  PuzzlePiece,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import type { BrowserView, HandoffView } from '../lib/types'
import { ago, bytes } from '../lib/format'
import { t } from '../lib/i18n'

/** Sizes offered for "small files stay in the browser". */
const SIZES = [1, 100, 500, 1024, 2048, 3072].map((mb) => mb * 1024 * 1024)

type Mode = 'all' | 'size' | 'off'

function modeOf(h: HandoffView): Mode {
  if (!h.enabled) return 'off'
  return h.minBytes === 0 ? 'all' : 'size'
}

function BrowserIcon({ b }: { b: BrowserView }) {
  return b.id === 'chrome' || b.id === 'chromium' ? (
    <GoogleChromeLogo size={22} aria-hidden />
  ) : (
    <Browser size={22} aria-hidden />
  )
}

/** One browser: what state the extension is in there, and the one thing to do next. */
function BrowserRow({
  b,
  onOpen,
}: {
  b: BrowserView
  onOpen: (page: 'store' | 'extensions') => void
}) {
  const status =
    b.extension === 'missing'
      ? { state: 'missing', label: t('Not added') }
      : b.extension === 'off'
        ? { state: 'off', label: t('Turned off') }
        : b.connected
          ? { state: 'connected', label: t('Connected') }
          : { state: 'added', label: t('Added') }
  const detail =
    b.extension === 'missing'
      ? t('Add it so downloads you start in {browser} come to Fuselane.', { browser: b.name })
      : b.extension === 'off'
        ? t('It is installed but turned off. Turn it on in {browser}’s extensions page.', {
            browser: b.name,
          })
        : b.connected && b.lastSeen
          ? b.version
            ? t('Checked in {when}, version {version}', {
                when: ago(b.lastSeen),
                version: b.version,
              })
            : t('Checked in {when}', { when: ago(b.lastSeen) })
          : t('Open {browser} once so it can check in.', { browser: b.name })
  return (
    <li className="ext-browser" data-state={status.state} data-browser={b.id}>
      <span className="ext-browser-ic">
        <BrowserIcon b={b} />
      </span>
      <div className="ext-browser-text">
        <p className="ext-browser-name" translate="no">
          {b.name}
        </p>
        <p className="muted">{detail}</p>
      </div>
      <span className="chip ext-chip" data-state={status.state}>
        {status.state === 'connected' ? (
          <PlugsConnected size={13} weight="bold" aria-hidden />
        ) : (
          <Plugs size={13} aria-hidden />
        )}
        {status.label}
      </span>
      <div className="ext-browser-action">
        {b.extension === 'missing' && (
          <button type="button" className="btn btn-primary" onClick={() => onOpen('store')}>
            {t('Add to {browser}', { browser: b.name })}
          </button>
        )}
        {b.extension === 'off' && (
          <button type="button" className="btn" onClick={() => onOpen('extensions')}>
            {t('Turn on')} <ArrowSquareOut size={14} aria-hidden />
          </button>
        )}
      </div>
    </li>
  )
}

/** The browsers here, with the extension's state in each; shared with the welcome tour. */
export function BrowserList({ compact = false }: { compact?: boolean }) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [list, setList] = useState<BrowserView[] | null>(null)
  const [waiting, setWaiting] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!backend) return
    try {
      setList(await backend.extensionBrowsers())
    } catch {
      setList([])
    }
  }, [backend])

  // Coming back from the browser is when the state changes: look again then, and
  // every few seconds for a while after sending someone to a store.
  useEffect(() => {
    void load()
    const on = () => void load()
    addEventListener('focus', on)
    return () => removeEventListener('focus', on)
  }, [load])
  useEffect(() => {
    if (!waiting) return
    const id = setInterval(() => void load(), 3000)
    const stop = setTimeout(() => setWaiting(null), 3 * 60_000)
    return () => {
      clearInterval(id)
      clearTimeout(stop)
    }
  }, [waiting, load])

  if (list === null)
    return (
      <ul className="ext-browsers" aria-busy="true" aria-label={t('Looking at your browsers')} />
    )
  if (!list.length)
    return (
      <p className="muted ext-empty">
        {t(
          'No supported browser found. Fuselane works with Chrome, Edge, Brave, Vivaldi and Firefox.',
        )}
      </p>
    )
  return (
    <ul
      className={`ext-browsers${compact ? ' compact' : ''}`}
      aria-label={t('Your browsers')}
      aria-live="polite"
    >
      {list.map((b) => (
        <BrowserRow
          key={b.id}
          b={b}
          onOpen={(page) => {
            if (page === 'store') setWaiting(b.id)
            void act((be) => be.extensionOpen(b.id, page))
          }}
        />
      ))}
    </ul>
  )
}

/** Where downloads started in the browser go: all to Fuselane, big ones only, or none. */
function HandoffSetting() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [h, setH] = useState<HandoffView | null>(null)

  useEffect(() => {
    void backend
      ?.handoff()
      .then(setH)
      .catch(() => {})
  }, [backend])

  if (!h) return null
  const mode = modeOf(h)
  const size = h.minBytes > 0 ? h.minBytes : SIZES[0]!
  const save = (next: HandoffView) => {
    setH(next)
    void act(async (b) => setH(await b.setHandoff(next)))
  }
  const options: { id: Mode; name: string; detail: string }[] = [
    {
      id: 'all',
      name: t('Everything to Fuselane'),
      detail: t('Every download from the browser comes here.'),
    },
    {
      id: 'size',
      name: t('Small files stay in the browser'),
      detail: t('Files up to the size below stay in the browser; bigger ones come here.'),
    },
    {
      id: 'off',
      name: t('Keep downloads in the browser'),
      detail: t('The extension stays quiet. Right-click a link to send one to Fuselane.'),
    },
  ]
  return (
    <fieldset className="ext-handoff">
      <legend className="setting-name">{t('Where browser downloads go')}</legend>
      <div className="ext-options" role="radiogroup" aria-label={t('Where browser downloads go')}>
        {options.map((o) => (
          <label key={o.id} className="ext-option" data-checked={mode === o.id}>
            <input
              type="radio"
              name="handoff"
              value={o.id}
              checked={mode === o.id}
              onChange={() =>
                save(
                  o.id === 'off'
                    ? { enabled: false, minBytes: h.minBytes }
                    : o.id === 'all'
                      ? { enabled: true, minBytes: 0 }
                      : { enabled: true, minBytes: size },
                )
              }
            />
            <span>
              <span className="ext-option-name">{o.name}</span>
              <span className="muted">{o.detail}</span>
            </span>
          </label>
        ))}
      </div>
      {mode === 'size' && (
        <label className="ext-size">
          <span>{t('Stay in the browser up to')}</span>
          <select
            value={SIZES.includes(size) ? size : SIZES[0]}
            onChange={(e) => save({ enabled: true, minBytes: Number(e.target.value) })}
          >
            {SIZES.map((s) => (
              <option key={s} value={s}>
                {bytes(s)}
              </option>
            ))}
          </select>
        </label>
      )}
    </fieldset>
  )
}

export function BrowsersView() {
  const info = useApp((s) => s.info)
  return (
    <section className="page" aria-labelledby="ext-title">
      <header className="page-head">
        <h1 id="ext-title">{t('Browser extension')}</h1>
      </header>
      <p className="page-lead muted">
        {t(
          'With the extension, downloads you start in your browser come to Fuselane by themselves and use every network.',
        )}
      </p>
      {info?.flatpak ? (
        <p className="muted" data-testid="extension-flatpak">
          {t(
            "The browser extension can't talk to the Flatpak version yet; use the .deb or AppImage for it.",
          )}
        </p>
      ) : (
        <>
          <BrowserList />
          <HandoffSetting />
        </>
      )}
    </section>
  )
}

const NUDGE_KEY = 'fuselane.extensionNudge'

/**
 * On Downloads, until it's dismissed: when no browser has the extension on, one
 * line offering to add it to the first browser that's missing it.
 */
export function ExtensionNudge() {
  const backend = useApp((s) => s.backend)
  const info = useApp((s) => s.info)
  const act = useApp((s) => s.act)
  const [list, setList] = useState<BrowserView[] | null>(null)
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(NUDGE_KEY) === 'dismissed'
    } catch {
      return false
    }
  })

  useEffect(() => {
    if (!backend || dismissed || info?.flatpak) return
    void backend
      .extensionBrowsers()
      .then(setList)
      .catch(() => setList([]))
  }, [backend, dismissed, info?.flatpak])

  if (dismissed || !list || list.some((b) => b.extension === 'on')) return null
  const target = list.find((b) => b.extension === 'missing') ?? list[0]
  if (!target) return null
  const dismiss = () => {
    setDismissed(true)
    try {
      localStorage.setItem(NUDGE_KEY, 'dismissed')
    } catch {
      // Private storage off: it comes back next launch, which is fine.
    }
  }
  return (
    <div className="update-banner ext-nudge" role="status" data-testid="extension-nudge">
      <PuzzlePiece size={18} aria-hidden className="ic-fuse" />
      <p>{t('Add the extension and downloads from your browser come here by themselves.')}</p>
      <button
        type="button"
        className="btn btn-primary"
        onClick={() =>
          void act((b) =>
            b.extensionOpen(target.id, target.extension === 'off' ? 'extensions' : 'store'),
          ).then(() => useApp.getState().setView('browsers'))
        }
      >
        {target.extension === 'off'
          ? t('Turn on in {browser}', { browser: target.name })
          : t('Add to {browser}', { browser: target.name })}
      </button>
      <button type="button" className="btn btn-ghost" onClick={dismiss}>
        {t('Not now')}
      </button>
    </div>
  )
}
