import { useEffect, useState } from 'react'
import { ArrowCircleUp, CheckCircle, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'

/** Checks quietly a little after launch, then offers the update without nagging. */
export function UpdateBanner() {
  const backend = useApp((s) => s.backend)
  const update = useApp((s) => s.update)
  const dismissed = useApp((s) => s.updateDismissed)
  const check = useApp((s) => s.checkUpdate)
  const dismiss = useApp((s) => s.dismissUpdate)
  const act = useApp((s) => s.act)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (!backend) return
    const first = setTimeout(() => void check(true), backend.demo ? 300 : 8000)
    const again = setInterval(() => void check(true), 6 * 60 * 60 * 1000)
    return () => {
      clearTimeout(first)
      clearInterval(again)
    }
  }, [backend, check])

  if (!update || dismissed) return null
  return (
    <div className="update-banner" role="status">
      <ArrowCircleUp size={18} aria-hidden className="ic-fuse" />
      <p>
        Fuselane <span className="num">{update.version}</span> is available.
        <span className="muted"> Downloads pause and continue after the restart.</span>
      </p>
      <button
        className="btn btn-primary"
        disabled={busy}
        onClick={async () => {
          setBusy(true)
          await act((b) => b.installUpdate())
          setBusy(false)
        }}
      >
        {busy ? 'Updating…' : 'Update and restart'}
      </button>
      <button className="icon-btn" aria-label="Later" title="Later" onClick={dismiss}>
        <X size={16} aria-hidden />
      </button>
    </div>
  )
}

/** Shown once after the app updated itself, so people know it worked. */
export function UpdatedBanner() {
  const info = useApp((s) => s.info)
  const act = useApp((s) => s.act)
  const [hidden, setHidden] = useState(false)
  if (!info?.updatedFrom || hidden) return null
  return (
    <div className="update-banner updated" role="status">
      <CheckCircle size={18} weight="fill" aria-hidden className="ic-success" />
      <p>
        Fuselane was updated to <span className="num">{info.version}</span>.
        <span className="muted"> Your downloads and settings are kept.</span>
      </p>
      <button className="btn" onClick={() => act((b) => b.openReleaseNotes())}>
        What&apos;s new
      </button>
      <button className="icon-btn" aria-label="Close" title="Close" onClick={() => setHidden(true)}>
        <X size={16} aria-hidden />
      </button>
    </div>
  )
}
