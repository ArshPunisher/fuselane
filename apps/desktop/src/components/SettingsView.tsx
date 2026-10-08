import { useState } from 'react'
import { useApp, type Theme } from '../lib/store'
import { LimitField } from './LimitField'

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
