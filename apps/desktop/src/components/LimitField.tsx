// A speed limit input: a number plus KB/s or MB/s. Empty means no limit.
import { useEffect, useId, useState } from 'react'

const UNITS = { KB: 1024, MB: 1024 * 1024 } as const
type Unit = keyof typeof UNITS

function split(rate: number): { text: string; unit: Unit } {
  if (!rate) return { text: '', unit: 'MB' }
  if (rate % UNITS.MB === 0 || rate >= 10 * UNITS.MB)
    return { text: String(+(rate / UNITS.MB).toFixed(2)), unit: 'MB' }
  return { text: String(Math.round(rate / UNITS.KB)), unit: 'KB' }
}

/** Parses what was typed: a rate in bytes/s, 0 for empty, or null if it isn't a speed. */
export function parseLimit(text: string, unit: Unit): number | null {
  const t = text.trim().replace(',', '.')
  if (!t) return 0
  if (!/^\d+(\.\d+)?$/.test(t)) return null
  const v = Number(t)
  return Number.isFinite(v) ? Math.round(v * UNITS[unit]) : null
}

export function LimitField({
  label,
  rate,
  onChange,
  hideLabel = false,
}: {
  label: string
  rate: number
  /** The new rate, or null while the text isn't a valid speed. */
  onChange: (rate: number | null) => void
  hideLabel?: boolean
}) {
  const id = useId()
  const [state, setState] = useState(() => split(rate))
  const parsed = parseLimit(state.text, state.unit)
  // Follow outside changes (a reset or a saved value) without fighting the user's typing.
  useEffect(() => {
    if (parseLimit(state.text, state.unit) !== rate) setState(split(rate))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rate])
  return (
    <div className="limit-field">
      <label htmlFor={id} className={hideLabel ? 'sr-only' : undefined}>
        {label}
      </label>
      <div className="limit-inputs">
        <input
          id={id}
          inputMode="decimal"
          autoComplete="off"
          placeholder="No limit"
          value={state.text}
          aria-invalid={parsed === null ? true : undefined}
          aria-describedby={parsed === null ? `${id}-err` : undefined}
          onChange={(e) => {
            const next = { ...state, text: e.target.value }
            setState(next)
            onChange(parseLimit(next.text, next.unit))
          }}
        />
        <select
          aria-label={`${label} unit`}
          value={state.unit}
          onChange={(e) => {
            const next = { ...state, unit: e.target.value as Unit }
            setState(next)
            onChange(parseLimit(next.text, next.unit))
          }}
        >
          <option value="KB">KB/s</option>
          <option value="MB">MB/s</option>
        </select>
      </div>
      {parsed === null && (
        <p id={`${id}-err`} className="field-error" aria-live="polite">
          Enter a number, like 5 or 2.5.
        </p>
      )}
    </div>
  )
}
