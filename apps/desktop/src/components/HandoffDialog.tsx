import { useEffect, useRef, useState } from 'react'
import { Desktop, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { JobView } from '../lib/types'

/**
 * Hand a paused download to another Fuselane on this network (B9.9): its
 * partial file and progress go there, and it carries on on that computer.
 */
export function HandoffDialog({
  job,
  open,
  onClose,
}: {
  job: JobView
  open: boolean
  onClose: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const backend = useApp((s) => s.backend)
  const nearby = useApp((s) => s.nearby)
  const [sending, setSending] = useState<string | null>(null)
  const [sent, setSent] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const devices = (nearby?.devices ?? []).filter((d) => d.fuselane)

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setSent(null)
      setProblem(null)
      d.showModal()
      // Nearby starts the first time it's needed (NEARBY.md).
      if (backend && !nearby?.on) void backend.nearbyStart().catch(() => {})
    }
    if (!open && d.open) d.close()
  }, [open, backend, nearby?.on])

  const send = async (fingerprint: string, alias: string) => {
    if (!backend) return
    setSending(fingerprint)
    setProblem(null)
    try {
      await backend.nearbyHandoff(job.id, fingerprint)
      setSent(alias)
    } catch (e) {
      const u = toUiError(e)
      setProblem([u.message, u.hint].filter(Boolean).join(' '))
    } finally {
      setSending(null)
    }
  }

  return (
    <dialog
      ref={ref}
      className="dialog handoff-dialog"
      aria-labelledby="ho-title"
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      {open && (
        <form className="handoff" method="dialog" onSubmit={(e) => e.preventDefault()}>
          <header className="dialog-head">
            <h2 id="ho-title">Continue on another computer</h2>
            <button type="button" className="icon-btn" aria-label="Close" onClick={onClose}>
              <X size={16} aria-hidden />
            </button>
          </header>
          {sent ? (
            <p className="dialog-text" role="status">
              Sent to <b translate="no">{sent}</b>. It appears there paused; press Resume on that
              computer to carry on. You can remove it here.
            </p>
          ) : (
            <>
              <p className="dialog-text">
                What&apos;s downloaded so far goes along, so the other computer carries on where
                this one stopped. Both need Fuselane, on the same network.
              </p>
              {devices.length ? (
                <ul className="handoff-devices">
                  {devices.map((d) => (
                    <li key={d.fingerprint}>
                      <button
                        type="button"
                        className="btn"
                        disabled={sending !== null}
                        onClick={() => void send(d.fingerprint, d.alias)}
                      >
                        <Desktop size={16} aria-hidden />
                        <span translate="no">{d.alias}</span>
                        {sending === d.fingerprint && <span className="muted">Sending…</span>}
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="muted" role="status">
                  Looking for other computers with Fuselane… Open Fuselane on the other computer.
                </p>
              )}
            </>
          )}
          {problem && (
            <p className="field-error" role="alert">
              {problem}
            </p>
          )}
          <footer className="dialog-foot">
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              {sent ? 'Done' : 'Cancel'}
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}
