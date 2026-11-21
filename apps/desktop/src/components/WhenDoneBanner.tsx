import { useEffect, useState } from 'react'
import { Moon, Power, SignOut } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { mark, t, tn, trn } from '../lib/i18n'

// Whole sentences: Hindi puts the countdown in the middle of the verb phrase.
const WORDS = {
  sleep: {
    one: mark('All downloads finished. Your computer goes to sleep in {n} second.'),
    other: mark('All downloads finished. Your computer goes to sleep in {n} seconds.'),
    Icon: Moon,
  },
  'shut-down': {
    one: mark('All downloads finished. Your computer shuts down in {n} second.'),
    other: mark('All downloads finished. Your computer shuts down in {n} seconds.'),
    Icon: Power,
  },
  quit: {
    one: mark('All downloads finished. Fuselane quits in {n} second.'),
    other: mark('All downloads finished. Fuselane quits in {n} seconds.'),
    Icon: SignOut,
  },
} as const

/**
 * A polite live region for screen readers. It is mounted empty and filled after,
 * because a region is announced when its text changes, not when it first appears.
 */
function Announce({ text }: { text: string }) {
  const [shown, setShown] = useState('')
  useEffect(() => setShown(text), [text])
  return (
    <p className="sr-only" role="status">
      {shown}
    </p>
  )
}

/** The countdown before a sleep, shut-down or quit, with one way out. */
export function WhenDoneBanner() {
  const whenDone = useApp((s) => s.whenDone)
  const cancel = useApp((s) => s.cancelWhenDone)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!whenDone) return
    const t = setInterval(() => setNow(Date.now()), 250)
    return () => clearInterval(t)
  }, [whenDone])
  if (!whenDone || whenDone.action === 'nothing') return null
  const left = Math.max(0, Math.ceil((whenDone.endsAt - now) / 1000))
  const { one, other, Icon } = WORDS[whenDone.action]
  // Screen readers hear it at 60, 30, 10 and 5 seconds, not every second.
  const spoken = left > 30 ? 60 : left > 10 ? 30 : left > 5 ? 10 : 5
  return (
    <div className="update-banner when-done">
      <Icon size={18} aria-hidden className="ic-fuse" />
      <p aria-hidden>{trn(left, one, other, { n: <span className="num">{left}</span> })}</p>
      <Announce text={tn(spoken, one, other)} />
      <button className="btn btn-primary" onClick={() => void cancel()}>
        {t('Cancel')}
      </button>
    </div>
  )
}
