// Fields that save themselves: a moment after typing stops, on leaving the
// field, or on Enter. A small tick says it worked; errors stay with the field.
import { useCallback, useEffect, useRef, useState } from 'react'
import { Check } from '@phosphor-icons/react'
import { t } from '../lib/i18n'
import { LimitField } from './LimitField'

export type SaveState = 'idle' | 'saving' | 'saved'

/** How long typing has to pause before it saves. */
const PAUSE = 900

/**
 * Saves `draft` when it differs from `saved` and passes `ok`, after a pause or
 * when `flush` is called (blur, Enter). `draft` is null while it isn't valid.
 */
export function useAutoSave<T>(
  draft: T | null,
  saved: T,
  save: (v: T) => Promise<boolean>,
  ok: (v: T) => boolean = () => true,
): { state: SaveState; flush: () => void } {
  const [state, setState] = useState<SaveState>('idle')
  const latest = useRef({ draft, saved, save, ok })
  latest.current = { draft, saved, save, ok }
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const flush = useCallback(() => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = null
    const { draft: d, saved: s, save: run, ok: valid } = latest.current
    if (d === null || Object.is(d, s) || !valid(d)) return
    setState('saving')
    void run(d).then((done) => setState(done ? 'saved' : 'idle'))
  }, [])
  useEffect(() => {
    if (draft === null || Object.is(draft, saved)) return
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(flush, PAUSE)
    return () => {
      if (timer.current) clearTimeout(timer.current)
    }
  }, [draft, saved, flush])
  // The tick fades after a while.
  useEffect(() => {
    if (state !== 'saved') return
    const done = setTimeout(() => setState('idle'), 2500)
    return () => clearTimeout(done)
  }, [state])
  return { state, flush }
}

/** "Saved" with a tick, announced politely; empty otherwise. */
export function SavedTick({ state, text }: { state: SaveState; text?: string | undefined }) {
  return (
    <span className="saved-tick" role="status" data-state={state}>
      {state === 'saved' && (
        <>
          <Check size={13} weight="bold" aria-hidden /> {text ?? t('Saved')}
        </>
      )}
      {state === 'saving' && t('Saving…')}
    </span>
  )
}

/** A speed limit that saves itself (0 = no limit). */
export function AutoLimit({
  label,
  hideLabel = false,
  rate,
  save,
  allowZero = true,
  savedText,
}: {
  label: string
  hideLabel?: boolean
  rate: number
  save: (rate: number) => Promise<boolean>
  allowZero?: boolean
  savedText?: ((rate: number) => string) | undefined
}) {
  const [draft, setDraft] = useState<number | null>(rate)
  useEffect(() => setDraft(rate), [rate])
  const { state, flush } = useAutoSave(draft, rate, save, (r) => allowZero || r > 0)
  return (
    <div
      className="auto-field"
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) flush()
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          flush()
        }
      }}
    >
      <LimitField label={label} hideLabel={hideLabel} rate={rate} onChange={setDraft} />
      <SavedTick state={state} text={savedText && draft !== null ? savedText(draft) : undefined} />
    </div>
  )
}
