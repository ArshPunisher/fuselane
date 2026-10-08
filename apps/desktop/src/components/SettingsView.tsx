import { useApp, type Theme } from '../lib/store'

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
