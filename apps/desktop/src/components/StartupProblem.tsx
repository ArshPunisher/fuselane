import { useEffect, useRef, useState, type RefObject } from 'react'
import {
  ArrowCircleUp,
  ArrowSquareOut,
  ArrowsClockwise,
  Copy,
  FolderOpen,
  WarningCircle,
} from '@phosphor-icons/react'
import type { StartupBackend } from '../lib/startup'
import type { UiError, UpdateProgress } from '../lib/types'
import { toUiError } from '../lib/backend'
import { bytes, eta, rateText } from '../lib/format'
import { t, tb, tr, useLocale } from '../lib/i18n'
import { applyTheme } from '../lib/store'
import { Wordmark } from './Wordmark'

/** The theme chosen in Settings, applied before the first paint (no flash of the other one). */
export function applySavedTheme() {
  try {
    const theme = localStorage.getItem('fuselane.theme')
    applyTheme(theme === 'light' || theme === 'dark' ? theme : 'system')
  } catch {
    applyTheme('system')
  }
}

/**
 * Shown instead of the app when the download list couldn't be opened at launch
 * (src-tauri/src/startup.rs). Launched from Finder or the Start menu nobody sees
 * the terminal, so without this the app would look like it does nothing.
 */
export function StartupProblemScreen({ backend }: { backend: StartupBackend }) {
  useLocale()
  const heading = useRef<HTMLHeadingElement>(null)

  useEffect(() => heading.current?.focus(), [])

  return (
    <div className="startup" data-kind={backend.problem.kind}>
      <header className="startup-top">
        <Wordmark />
        {backend.demo && (
          <span className="chip" title={t('Running in a browser: these transfers are simulated.')}>
            {t('Demo data')}
          </span>
        )}
      </header>
      <main className="startup-main" id="main">
        {backend.problem.kind === 'newer' ? (
          <Newer backend={backend} heading={heading} />
        ) : (
          <Other backend={backend} heading={heading} />
        )}
      </main>
    </div>
  )
}

type Step =
  | { s: 'idle' }
  | { s: 'checking' }
  | { s: 'downloading'; version: string; size: number | null; progress: UpdateProgress | null }
  | { s: 'installing'; version: string }
  | { s: 'none' }
  | { s: 'failed'; error: UiError }

function Newer({
  backend,
  heading,
}: {
  backend: StartupBackend
  heading: RefObject<HTMLHeadingElement | null>
}) {
  const [step, setStep] = useState<Step>({ s: 'idle' })
  const busy = step.s === 'checking' || step.s === 'downloading' || step.s === 'installing'
  const stuck = step.s === 'none' || step.s === 'failed'

  const update = async () => {
    setStep({ s: 'checking' })
    let found
    try {
      found = await backend.checkUpdate()
    } catch (e) {
      setStep({ s: 'failed', error: toUiError(e) })
      return
    }
    if (!found) {
      setStep({ s: 'none' })
      return
    }
    const { version, size } = found
    setStep({ s: 'downloading', version, size, progress: null })
    try {
      await backend.installUpdate((progress) =>
        setStep(
          progress.phase === 'installing'
            ? { s: 'installing', version }
            : { s: 'downloading', version, size, progress },
        ),
      )
    } catch (e) {
      const error = toUiError(e)
      setStep(error.code === 'update-cancelled' ? { s: 'idle' } : { s: 'failed', error })
    }
  }

  return (
    <section className="startup-panel" aria-labelledby="startup-title">
      <ArrowCircleUp size={28} aria-hidden className="ic-fuse" />
      <h1 id="startup-title" ref={heading} tabIndex={-1}>
        {t('Your download list is from a newer Fuselane')}
      </h1>
      <p className="startup-body">
        {tr('This copy is version {version}. Update to keep going. Your downloads are kept.', {
          version: <span className="num">{backend.problem.version}</span>,
        })}
      </p>

      <div className="startup-status" role="status" aria-live="polite">
        {step.s === 'checking' && <p className="muted">{t('Looking for the update…')}</p>}
        {step.s === 'downloading' && <Progress step={step} />}
        {step.s === 'installing' && (
          <p className="startup-line">
            <ArrowsClockwise size={16} aria-hidden className="ic-fuse" />
            <span>
              {tr('Installing {version}.', {
                version: <span className="num">{step.version}</span>,
              })}{' '}
              <span className="muted">
                {t('Fuselane restarts in a moment and your downloads carry on.')}
              </span>
            </span>
          </p>
        )}
        {step.s === 'none' && (
          <div className="startup-note">
            <p>{t('No update was found for this copy.')}</p>
            <p className="muted">
              {t(
                'Get the newest Fuselane from fuselane.app and install it over this one. Your downloads are kept.',
              )}
            </p>
          </div>
        )}
      </div>
      {step.s === 'failed' && (
        <div className="startup-note" role="alert" data-tone="danger">
          <p>{step.error.message}</p>
          {step.error.hint && <p className="muted">{step.error.hint}</p>}
          <p className="muted">
            {t('You can also get the newest Fuselane from fuselane.app. Your downloads are kept.')}
          </p>
        </div>
      )}

      <div className="startup-actions">
        {step.s === 'downloading' ? (
          <button className="btn" onClick={() => void backend.cancelUpdate()}>
            {t('Cancel')}
          </button>
        ) : stuck ? (
          <>
            <button className="btn btn-primary" onClick={() => void backend.getFuselane()}>
              <ArrowSquareOut size={16} aria-hidden />
              {t('Get Fuselane from fuselane.app')}
            </button>
            {step.s === 'failed' && (
              <button className="btn" onClick={() => void update()}>
                {t('Try again')}
              </button>
            )}
          </>
        ) : (
          <button className="btn btn-primary" disabled={busy} onClick={() => void update()}>
            {t('Update now')}
          </button>
        )}
        {step.s !== 'installing' && (
          <button className="btn btn-ghost" onClick={() => void backend.quit()}>
            {t('Quit')}
          </button>
        )}
      </div>
    </section>
  )
}

function Progress({ step }: { step: Extract<Step, { s: 'downloading' }> }) {
  const p = step.progress
  const total = p?.total ?? step.size
  const done = p?.done ?? 0
  const pct = total ? Math.min(100, (done / total) * 100) : 0
  const left = total && p ? eta(total - done, p.rate) : ''
  return (
    <div className="startup-progress">
      <p>
        {tr('Downloading Fuselane {version}', {
          version: <span className="num">{step.version}</span>,
        })}
        {p && p.networks > 1 && (
          <span className="muted"> {t('over {n} networks', { n: p.networks })}</span>
        )}
      </p>
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
      <span className="num muted">
        {total ? t('{done} of {total}', { done: bytes(done), total: bytes(total) }) : bytes(done)}
        {p && p.rate > 0 && `, ${rateText(p.rate)}`}
        {left && `, ${left}`}
      </span>
    </div>
  )
}

function Other({
  backend,
  heading,
}: {
  backend: StartupBackend
  heading: RefObject<HTMLHeadingElement | null>
}) {
  const p = backend.problem
  const [copied, setCopied] = useState(false)
  const [error, setError] = useState<UiError | null>(null)
  const act = async (f: () => Promise<void>) => {
    setError(null)
    try {
      await f()
    } catch (e) {
      setError(toUiError(e))
    }
  }
  const copy = () =>
    act(async () => {
      await backend.copyDetails()
      setCopied(true)
    })

  return (
    <section className="startup-panel" aria-labelledby="startup-title">
      <WarningCircle size={28} aria-hidden className="ic-danger" />
      <h1 id="startup-title" ref={heading} tabIndex={-1}>
        {t("Fuselane can't start")}
      </h1>
      <p className="startup-body">{tb(p.message)}</p>
      {p.hint && <p className="startup-body muted">{tb(p.hint)}</p>}
      <pre className="startup-detail" aria-label={t('Error details')} translate="no">
        {p.detail}
      </pre>
      {error && (
        <div className="startup-note" role="alert" data-tone="danger">
          <p>{error.message}</p>
          {error.hint && <p className="muted">{error.hint}</p>}
        </div>
      )}
      <div className="startup-actions">
        {p.home && (
          <button className="btn btn-primary" onClick={() => void act(backend.revealHome)}>
            <FolderOpen size={16} aria-hidden />
            {t('Open the folder')}
          </button>
        )}
        <button className="btn" onClick={() => void copy()}>
          <Copy size={16} aria-hidden />
          {copied ? t('Copied') : t('Copy details')}
        </button>
        <button className="btn btn-ghost" onClick={() => void act(backend.quit)}>
          {t('Quit')}
        </button>
      </div>
      <span className="sr-only" role="status">
        {copied ? t('Copied') : ''}
      </span>
    </section>
  )
}
