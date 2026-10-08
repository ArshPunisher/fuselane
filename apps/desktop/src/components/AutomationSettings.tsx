import { useEffect, useState } from 'react'
import { useApp } from '../lib/store'
import type { Automation, WhenDone } from '../lib/types'

const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
const DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']

/** 75 → "01:15", for <input type="time">. */
function hhmm(minutes: number): string {
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`
}

function minutesOf(value: string): number | null {
  const m = /^(\d{2}):(\d{2})$/.exec(value)
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  return h < 24 && min < 60 ? h * 60 + min : null
}

/** Saves one change to the automation settings. */
function useAutomation(): [Automation | null, (next: Automation) => Promise<boolean>] {
  const view = useApp((s) => s.automation)
  const save = useApp((s) => s.setAutomation)
  return [view?.settings ?? null, save]
}

export function DownloadsAtOnceSetting() {
  const backend = useApp((s) => s.backend)
  const [n, setN] = useState<number | null>(null)
  const [status, setStatus] = useState('')
  useEffect(() => {
    void backend
      ?.maxRunning()
      .then(setN)
      .catch(() => setN(3))
  }, [backend])
  async function change(next: number) {
    if (!backend || next < 1 || next > 8) return
    setStatus('')
    try {
      setN(await backend.setMaxRunning(next))
      setStatus(next === 1 ? 'One at a time.' : `Up to ${next} at once.`)
    } catch {
      setStatus("Couldn't save that. Try again.")
    }
  }
  return (
    <div className="setting">
      <div>
        <p className="setting-name" id="at-once-label">
          Downloads at once
        </p>
        <p className="muted">Each one already uses every network. More start as others finish.</p>
        <p className="muted" role="status">
          {status}
        </p>
      </div>
      <div className="stepper" role="group" aria-labelledby="at-once-label">
        <button
          type="button"
          className="btn"
          aria-label="One fewer"
          disabled={n === null || n <= 1}
          onClick={() => n !== null && void change(n - 1)}
        >
          −
        </button>
        <output className="num stepper-value" aria-live="polite">
          {n ?? ''}
        </output>
        <button
          type="button"
          className="btn"
          aria-label="One more"
          disabled={n === null || n >= 8}
          onClick={() => n !== null && void change(n + 1)}
        >
          +
        </button>
      </div>
    </div>
  )
}

export function ScheduleSetting() {
  const [settings, save] = useAutomation()
  const view = useApp((s) => s.automation)
  const [start, setStart] = useState('01:00')
  const [stop, setStop] = useState('07:00')
  const [days, setDays] = useState<boolean[]>(Array(7).fill(true))
  const [error, setError] = useState('')
  // Re-read only when the saved schedule itself changes, so changing another
  // setting never wipes times or days still being edited.
  const savedKey = settings
    ? `${settings.schedule.start}-${settings.schedule.stop}-${settings.schedule.days.join()}`
    : ''
  useEffect(() => {
    if (!settings) return
    setStart(hhmm(settings.schedule.start))
    setStop(hhmm(settings.schedule.stop))
    setDays(settings.schedule.days)
  }, [savedKey])
  if (!settings) return null
  const s = settings.schedule
  const a = minutesOf(start)
  const b = minutesOf(stop)
  const changed = a !== s.start || b !== s.stop || days.some((d, i) => d !== s.days[i])
  const enabled = s.enabled

  async function apply(next: Partial<Automation['schedule']>) {
    if (!settings) return
    setError('')
    const ok = await save({ ...settings, schedule: { ...settings.schedule, ...next } })
    if (!ok) setError('Not saved. Check the times and days.')
  }

  return (
    <form
      className="setting setting-stack"
      aria-label="Download schedule"
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        if (a === null || b === null) {
          setError('Enter times like 01:00.')
          return
        }
        void apply({ start: a, stop: b, days })
      }}
    >
      <div className="setting-row">
        <div>
          <p className="setting-name">Schedule</p>
          <p className="muted">
            Only download between two times, for example at night when data is cheap. Downloads
            outside it wait, and running ones pause and carry on next time.
          </p>
          <p className="muted" role="status">
            {enabled && view?.next ? view.next : ''}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          className="switch"
          aria-label="Download only on a schedule"
          aria-checked={enabled}
          onClick={() => void apply({ enabled: !enabled })}
        />
      </div>
      {enabled && (
        <div className="schedule-fields">
          <div className="field">
            <label htmlFor="sched-start">From</label>
            <input
              id="sched-start"
              name="start"
              type="time"
              value={start}
              onChange={(e) => setStart(e.target.value)}
            />
          </div>
          <div className="field">
            <label htmlFor="sched-stop">Until</label>
            <input
              id="sched-stop"
              name="stop"
              type="time"
              value={stop}
              onChange={(e) => setStop(e.target.value)}
            />
          </div>
          <fieldset className="days">
            <legend>Days</legend>
            {DAYS.map((d, i) => (
              <button
                key={d}
                type="button"
                className="day"
                aria-pressed={days[i]}
                aria-label={DAY_NAMES[i]}
                onClick={() => setDays(days.map((x, j) => (j === i ? !x : x)))}
              >
                {d}
              </button>
            ))}
          </fieldset>
          <button type="submit" className="btn" disabled={!changed}>
            Save
          </button>
          {a !== null && b !== null && b < a && (
            <p className="field-help schedule-note">
              Runs overnight: from {start} until {stop} the next morning.
            </p>
          )}
          {error && (
            <p className="field-error schedule-note" role="alert">
              {error}
            </p>
          )}
        </div>
      )}
    </form>
  )
}

const WHEN_DONE: { id: WhenDone; label: string }[] = [
  { id: 'nothing', label: 'Nothing' },
  { id: 'sleep', label: 'Sleep' },
  { id: 'shut-down', label: 'Shut down' },
  { id: 'quit', label: 'Quit' },
]

export function WhenDoneSetting() {
  const [settings, save] = useAutomation()
  if (!settings) return null
  const current = settings.whenDone
  const pick = (id: WhenDone) => void save({ ...settings, whenDone: id })
  return (
    <div className="setting">
      <div>
        <p className="setting-name" id="when-done-label">
          When everything finishes
        </p>
        <p className="muted">
          {current === 'nothing'
            ? 'Fuselane just waits.'
            : 'You get 60 seconds and a notification to cancel first.'}
        </p>
      </div>
      <div
        className="segmented"
        role="radiogroup"
        aria-labelledby="when-done-label"
        onKeyDown={(e) => {
          const i = WHEN_DONE.findIndex((w) => w.id === current)
          const step =
            e.key === 'ArrowRight' || e.key === 'ArrowDown'
              ? 1
              : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
                ? -1
                : 0
          if (!step) return
          e.preventDefault()
          const next = WHEN_DONE[(i + step + WHEN_DONE.length) % WHEN_DONE.length]
          if (next) {
            pick(next.id)
            e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${next.id}"]`)?.focus()
          }
        }}
      >
        {WHEN_DONE.map((w) => (
          <button
            key={w.id}
            data-id={w.id}
            role="radio"
            aria-checked={current === w.id}
            tabIndex={current === w.id ? 0 : -1}
            onClick={() => pick(w.id)}
          >
            {w.label}
          </button>
        ))}
      </div>
    </div>
  )
}

function ToggleSetting({
  name,
  help,
  on,
  onChange,
}: {
  name: string
  help: string
  on: boolean
  onChange: (on: boolean) => void
}) {
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{name}</p>
        <p className="muted">{help}</p>
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-label={name}
        aria-checked={on}
        onClick={() => onChange(!on)}
      />
    </div>
  )
}

export function KeepAwakeSetting() {
  const [settings, save] = useAutomation()
  if (!settings) return null
  return (
    <ToggleSetting
      name="Keep the computer awake"
      help="While something downloads, your computer won't go to sleep. The screen can still turn off."
      on={settings.keepAwake}
      onChange={(keepAwake) => void save({ ...settings, keepAwake })}
    />
  )
}

export function SortSetting() {
  const [settings, save] = useAutomation()
  const info = useApp((s) => s.info)
  if (!settings) return null
  return (
    <ToggleSetting
      name="Sort into folders by type"
      help={`Finished files go into Video, Music, Documents, Compressed and similar folders inside ${info?.defaultDir ?? 'your downloads folder'}. A folder you pick yourself is left alone.`}
      on={settings.sortByType}
      onChange={(sortByType) => void save({ ...settings, sortByType })}
    />
  )
}
