import { useEffect, useState } from 'react'
import {
  ArrowCircleDown,
  ArrowCircleUp,
  ArrowsClockwise,
  CheckCircle,
  Warning,
  X,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, eta, rateText } from '../lib/format'
import { t, tr } from '../lib/i18n'

/** Checks quietly a little after launch, then offers the update without nagging. */
export function UpdateBanner() {
  const backend = useApp((s) => s.backend)
  const update = useApp((s) => s.update)
  const dismissed = useApp((s) => s.updateDismissed)
  const check = useApp((s) => s.checkUpdate)
  const dismiss = useApp((s) => s.dismissUpdate)
  const install = useApp((s) => s.installUpdate)
  const cancel = useApp((s) => s.cancelUpdate)
  const progress = useApp((s) => s.updateProgress)
  const error = useApp((s) => s.updateError)

  useEffect(() => {
    if (!backend) return
    const first = setTimeout(() => void check(true), backend.demo ? 300 : 8000)
    const again = setInterval(() => void check(true), 6 * 60 * 60 * 1000)
    return () => {
      clearTimeout(first)
      clearInterval(again)
    }
  }, [backend, check])

  if (!update || (dismissed && !progress)) return null
  const version = <span className="num">{update.version}</span>

  if (progress?.phase === 'installing')
    return (
      <div className="update-banner" role="status" data-state="installing">
        <ArrowsClockwise size={18} aria-hidden className="ic-fuse" />
        <p>
          {tr('Installing {version}.', { version })}
          <span className="muted">
            {' '}
            {t('Fuselane restarts in a moment and your downloads carry on.')}
          </span>
        </p>
      </div>
    )

  if (progress) {
    const total = progress.total ?? update.size
    const pct = total ? Math.min(100, (progress.done / total) * 100) : 0
    const left = total ? eta(total - progress.done, progress.rate) : ''
    return (
      <div className="update-banner" role="status" data-state="downloading">
        <ArrowCircleDown size={18} aria-hidden className="ic-fuse" />
        <div className="update-text">
          <p>
            {tr('Downloading Fuselane {version}', { version })}
            {progress.networks > 1 && (
              <span className="muted"> {t('over {n} networks', { n: progress.networks })}</span>
            )}
          </p>
          <div className="update-progress">
            <div
              className="bar"
              data-status="running"
              role="progressbar"
              aria-label={t('Update download')}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(pct)}
            >
              <div className="bar-fill" style={{ width: `${pct}%` }} />
            </div>
            <span className="num">
              {total
                ? t('{done} of {total}', { done: bytes(progress.done), total: bytes(total) })
                : bytes(progress.done)}
              {progress.rate > 0 && `, ${rateText(progress.rate)}`}
              {left && `, ${left}`}
            </span>
          </div>
        </div>
        <button className="btn btn-ghost" onClick={() => void cancel()}>
          {t('Cancel')}
        </button>
      </div>
    )
  }

  if (error)
    return (
      <div className="update-banner" role="alert" data-state="failed">
        <Warning size={18} aria-hidden className="ic-fuse" />
        <p>
          {error.message}
          {error.hint && <span className="muted"> {error.hint}</span>}
        </p>
        <button className="btn" onClick={() => void install()}>
          {t('Try again')}
        </button>
        <button className="icon-btn" aria-label={t('Later')} title={t('Later')} onClick={dismiss}>
          <X size={16} aria-hidden />
        </button>
      </div>
    )

  return (
    <div className="update-banner" role="status" data-state="available">
      <ArrowCircleUp size={18} aria-hidden className="ic-fuse" />
      <p>
        {update.size
          ? tr('Fuselane {version} is ready to download, {size}.', {
              version,
              size: <span className="num">{bytes(update.size)}</span>,
            })
          : tr('Fuselane {version} is ready to download.', { version })}
        <span className="muted"> {t('Downloads pause and continue after the restart.')}</span>
      </p>
      <button className="btn btn-primary" onClick={() => void install()}>
        {t('Update and restart')}
      </button>
      <button className="icon-btn" aria-label={t('Later')} title={t('Later')} onClick={dismiss}>
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
        {tr('Fuselane was updated to {version}.', {
          version: <span className="num">{info.version}</span>,
        })}
        <span className="muted"> {t('Your downloads and settings are kept.')}</span>
      </p>
      <button className="btn" onClick={() => act((b) => b.openReleaseNotes())}>
        {t("What's new")}
      </button>
      <button
        className="icon-btn"
        aria-label={t('Close')}
        title={t('Close')}
        onClick={() => setHidden(true)}
      >
        <X size={16} aria-hidden />
      </button>
    </div>
  )
}
