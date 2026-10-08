import { useEffect, useState } from 'react'
import { Moon, Power, SignOut } from '@phosphor-icons/react'
import { useApp } from '../lib/store'

const WORDS = {
  sleep: { what: 'Your computer goes to sleep', Icon: Moon },
  'shut-down': { what: 'Your computer shuts down', Icon: Power },
  quit: { what: 'Fuselane quits', Icon: SignOut },
} as const

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
  const { what, Icon } = WORDS[whenDone.action]
  return (
    <div className="update-banner when-done" role="alert">
      <Icon size={18} aria-hidden className="ic-fuse" />
      <p>
        All downloads finished. {what} in <span className="num">{left}</span>{' '}
        {left === 1 ? 'second' : 'seconds'}.
      </p>
      <button className="btn btn-primary" onClick={() => void cancel()} autoFocus>
        Cancel
      </button>
    </div>
  )
}
