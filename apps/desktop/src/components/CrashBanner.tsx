import { useEffect, useState } from 'react'
import { Warning, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'

/**
 * After a crash, once: say so and offer to report it. The report opens as a
 * GitHub issue the person reads before sending; Fuselane sends nothing itself.
 */
export function CrashBanner() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [crash, setCrash] = useState<string | null>(null)
  useEffect(() => {
    void backend
      ?.unseenCrash()
      .then(setCrash)
      .catch(() => {})
  }, [backend])
  if (!crash) return null
  return (
    <div className="update-banner crash-banner" role="status">
      <Warning size={18} weight="fill" aria-hidden />
      <span>
        Fuselane closed unexpectedly last time. Sending a report helps fix it; you see everything it
        says before it&apos;s sent.
      </span>
      <button
        className="btn btn-sm"
        onClick={() =>
          void act(async (b) => {
            await b.reportProblem(crash)
            setCrash(null)
          })
        }
      >
        Report it
      </button>
      <button className="icon-btn" aria-label="Dismiss" onClick={() => setCrash(null)}>
        <X size={16} aria-hidden />
      </button>
    </div>
  )
}
