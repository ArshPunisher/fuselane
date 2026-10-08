import { useEffect, useState } from 'react'
import { useApp, type Theme } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { SeedSettings, UiError } from '../lib/types'
import { LimitField } from './LimitField'
import { SlowToggle } from './SlowMode'

const THEMES: { id: Theme; label: string }[] = [
  { id: 'system', label: 'System' },
  { id: 'light', label: 'Light' },
  { id: 'dark', label: 'Dark' },
]

export function ThemePicker() {
  const theme = useApp((s) => s.theme)
  const setTheme = useApp((s) => s.setTheme)
  return (
    <div
      className="segmented"
      role="radiogroup"
      aria-label="Theme"
      onKeyDown={(e) => {
        // Arrow keys move the choice, as in a native radio group.
        const i = THEMES.findIndex((t) => t.id === theme)
        const step =
          e.key === 'ArrowRight' || e.key === 'ArrowDown'
            ? 1
            : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
              ? -1
              : 0
        if (!step) return
        e.preventDefault()
        const next = THEMES[(i + step + THEMES.length) % THEMES.length]
        if (next) {
          setTheme(next.id)
          e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${next.id}"]`)?.focus()
        }
      }}
    >
      {THEMES.map((t) => (
        <button
          key={t.id}
          data-id={t.id}
          role="radio"
          aria-checked={theme === t.id}
          tabIndex={theme === t.id ? 0 : -1}
          onClick={() => setTheme(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  )
}

function SpeedLimitSetting() {
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  const [draft, setDraft] = useState<number | null>(limits.global)
  const [status, setStatus] = useState('')
  const changed = draft !== null && draft !== limits.global
  return (
    <form
      className="setting"
      aria-label="Speed limit"
      onSubmit={async (e) => {
        e.preventDefault()
        if (draft === null) return
        setStatus('')
        if (await save({ ...limits, global: draft }))
          setStatus(draft ? 'Saved. Running downloads follow it now.' : 'Limit removed.')
      }}
    >
      <div>
        <p className="setting-name">Speed limit</p>
        <p className="muted">For all downloads and networks together.</p>
        <p className="muted" role="status">
          {status}
        </p>
      </div>
      <div className="setting-control">
        <LimitField
          label="Speed limit for all networks"
          hideLabel
          rate={limits.global}
          onChange={setDraft}
        />
        <button type="submit" className="btn" disabled={!changed}>
          Save
        </button>
      </div>
    </form>
  )
}

function SlowModeSetting() {
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  const [draft, setDraft] = useState<number | null>(limits.slowRate)
  const [status, setStatus] = useState('')
  const changed = draft !== null && draft > 0 && draft !== limits.slowRate
  return (
    <form
      className="setting setting-stack"
      aria-label="Slow mode"
      onSubmit={async (e) => {
        e.preventDefault()
        if (draft === null || draft <= 0) return
        setStatus('')
        if (await save({ ...limits, slowRate: draft })) setStatus('Saved.')
      }}
    >
      <div className="setting-row">
        <div>
          <p className="setting-name">Slow mode</p>
          <p className="muted">
            One switch for calls and streaming: caps all downloads, then puts your normal limits
            back.
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <SlowToggle labelled={false} />
      </div>
      <div className="setting-control">
        <LimitField label="Slow mode speed" rate={limits.slowRate} onChange={setDraft} />
        <button type="submit" className="btn" disabled={!changed}>
          Save
        </button>
      </div>
    </form>
  )
}

const MOD = /Mac/i.test(navigator.platform) ? '⌘' : 'Ctrl'
const SHORTCUTS: [string[], string][] = [
  [[MOD, 'N'], 'New download (or paste a link anywhere)'],
  [[MOD, '1'], 'Downloads'],
  [[MOD, '2'], 'Networks'],
  [[MOD, '3'], 'Settings'],
  [['↑', '↓'], 'Move through downloads'],
  [['Space'], 'Pause or resume the selected download'],
  [['Esc'], 'Back to the list, or close a dialog'],
]

function ShortcutsSetting() {
  return (
    <div className="setting setting-stack">
      <p className="setting-name">Keyboard shortcuts</p>
      <dl className="shortcuts">
        {SHORTCUTS.map(([keys, what]) => (
          <div key={what}>
            <dt>
              {keys.map((k) => (
                <kbd key={k}>{k}</kbd>
              ))}
            </dt>
            <dd>{what}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

/** Sharing back after a torrent finishes: off by default, with two stop limits. */
function SharingSetting() {
  const backend = useApp((s) => s.backend)
  const [saved, setSaved] = useState<SeedSettings | null>(null)
  const [ratio, setRatio] = useState('')
  const [minutes, setMinutes] = useState('')
  const [error, setError] = useState<UiError | null>(null)
  const [status, setStatus] = useState('')
  useEffect(() => {
    void backend
      ?.seedSettings()
      .then((s) => {
        setSaved(s)
        setRatio(String(s.ratio))
        setMinutes(String(s.minutes))
      })
      .catch(() => setSaved({ enabled: false, ratio: 1, minutes: 60 }))
  }, [backend])

  async function save(next: SeedSettings) {
    if (!backend) return
    setError(null)
    setStatus('')
    try {
      const s = await backend.setSeedSettings(next)
      setSaved(s)
      setRatio(String(s.ratio))
      setMinutes(String(s.minutes))
      setStatus(s.enabled ? 'Saved.' : 'Sharing is off. Finished torrents stop at once.')
    } catch (e) {
      setError(toUiError(e))
    }
  }

  const r = Number(ratio)
  const m = Number(minutes)
  const changed = saved !== null && (r !== saved.ratio || m !== saved.minutes)
  const ratioError = error?.code === 'bad-ratio' ? error : null
  const minutesError = error?.code === 'bad-minutes' ? error : null
  return (
    <form
      className="setting setting-stack"
      aria-label="Share torrents after downloading"
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        if (saved) void save({ ...saved, ratio: r, minutes: m })
      }}
    >
      <div className="setting-row">
        <div>
          <p className="setting-name">Share torrents after downloading</p>
          <p className="muted">
            Uploads to other people for a while, then stops. Never over a phone tether or cellular
            while torrents are only sharing.
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          className="switch"
          aria-label="Share torrents after downloading"
          aria-checked={saved?.enabled === true}
          disabled={saved === null}
          onClick={() => saved && void save({ ...saved, enabled: !saved.enabled })}
        />
      </div>
      {saved?.enabled && (
        <div className="share-limits">
          <div className="field">
            <label htmlFor="share-ratio">Stop at ratio</label>
            <input
              id="share-ratio"
              name="ratio"
              type="number"
              inputMode="decimal"
              min={0.1}
              max={10}
              step={0.1}
              value={ratio}
              aria-invalid={ratioError ? true : undefined}
              aria-describedby={ratioError ? 'share-ratio-err' : 'share-ratio-help'}
              onChange={(e) => setRatio(e.target.value)}
            />
            {ratioError ? (
              <p id="share-ratio-err" className="field-error">
                {ratioError.message} {ratioError.hint}
              </p>
            ) : (
              <p id="share-ratio-help" className="field-help">
                1 means upload as much as you downloaded.
              </p>
            )}
          </div>
          <div className="field">
            <label htmlFor="share-minutes">Stop after (minutes)</label>
            <input
              id="share-minutes"
              name="minutes"
              type="number"
              inputMode="numeric"
              min={1}
              max={10080}
              step={1}
              value={minutes}
              aria-invalid={minutesError ? true : undefined}
              aria-describedby={minutesError ? 'share-minutes-err' : 'share-minutes-help'}
              onChange={(e) => setMinutes(e.target.value)}
            />
            {minutesError ? (
              <p id="share-minutes-err" className="field-error">
                {minutesError.message}
              </p>
            ) : (
              <p id="share-minutes-help" className="field-help">
                Whichever limit comes first ends sharing.
              </p>
            )}
          </div>
          <button type="submit" className="btn" disabled={!changed}>
            Save
          </button>
        </div>
      )}
    </form>
  )
}

function LookupSetting() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [on, setOn] = useState<boolean | null>(null)
  useEffect(() => {
    void backend
      ?.perNetworkDns()
      .then(setOn)
      .catch(() => setOn(false))
  }, [backend])
  return (
    <div className="setting">
      <div>
        <p className="setting-name">Look up servers through each network</p>
        <p className="muted">
          Faster when your networks are from different providers: each one gets a server near it.
          Uses Cloudflare and Google DNS, which then see the names of the sites you download from
          (never the files).
        </p>
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-label="Look up servers through each network"
        aria-checked={on === true}
        disabled={on === null}
        onClick={() => void act(async (b) => setOn(await b.setPerNetworkDns(!on)))}
      />
    </div>
  )
}

function UpdateSetting() {
  const check = useApp((s) => s.checkUpdate)
  const update = useApp((s) => s.update)
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  return (
    <div className="setting">
      <div>
        <p className="setting-name">Updates</p>
        <p className="muted">Fuselane checks a signed update feed. Nothing about you is sent.</p>
        <p className="muted" role="status">
          {status}
        </p>
      </div>
      <button
        type="button"
        className="btn"
        disabled={busy}
        onClick={async () => {
          setBusy(true)
          setStatus('')
          const r = await check(false)
          setBusy(false)
          if (r === 'current') setStatus("You're up to date.")
          if (r === 'available')
            setStatus(`Version ${useApp.getState().update?.version ?? ''} is ready to install.`)
        }}
      >
        {busy ? 'Checking…' : update ? 'Check again' : 'Check for updates'}
      </button>
    </div>
  )
}

function DiagnosticsSetting() {
  const backend = useApp((s) => s.backend)
  const [report, setReport] = useState('')
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  async function copy() {
    if (!backend || busy) return
    setBusy(true)
    setStatus('')
    try {
      const text = await backend.diagnostics()
      setReport(text)
      try {
        await navigator.clipboard.writeText(text)
        setStatus('Copied. Paste it into your bug report.')
      } catch {
        setStatus('Select the text below and copy it.')
      }
    } catch {
      setStatus("Couldn't build the report. Try again.")
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div>
          <p className="setting-name">Diagnostics</p>
          <p className="muted">
            For bug reports. It never includes IP addresses, links or file names, and Fuselane sends
            nothing by itself.
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <button type="button" className="btn" onClick={copy} disabled={busy}>
          {busy ? 'Collecting…' : 'Copy diagnostics'}
        </button>
      </div>
      {report && (
        <textarea
          className="report"
          readOnly
          value={report}
          aria-label="Diagnostics report"
          spellCheck={false}
          rows={10}
        />
      )}
    </div>
  )
}

export function SettingsView() {
  const info = useApp((s) => s.info)
  const demo = useApp((s) => s.backend?.demo)
  return (
    <section className="page" aria-labelledby="set-title">
      <header className="page-head">
        <h1 id="set-title">Settings</h1>
      </header>
      <div className="setting">
        <div>
          <p className="setting-name">Appearance</p>
          <p className="muted">Follows your system unless you pick one.</p>
        </div>
        <ThemePicker />
      </div>
      <SpeedLimitSetting />
      <div className="setting">
        <div>
          <p className="setting-name">Downloads folder</p>
          <p className="muted num">{info?.defaultDir ?? ''}</p>
        </div>
      </div>
      <SlowModeSetting />
      <SharingSetting />
      <LookupSetting />
      <UpdateSetting />
      <ShortcutsSetting />
      <DiagnosticsSetting />
      <div className="setting">
        <div>
          <p className="setting-name">Version</p>
          <p className="muted num">
            {info?.version ?? ''}
            {demo ? ', demo data' : ''}
          </p>
        </div>
      </div>
    </section>
  )
}
