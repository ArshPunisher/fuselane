import { Component, type ErrorInfo, type ReactNode } from 'react'
import { ArrowClockwise, Copy, WarningCircle } from '@phosphor-icons/react'
import { t } from '../lib/i18n'
import { Wordmark } from './Wordmark'

interface State {
  /** A separate flag: anything can be thrown, including null or 0. */
  failed: boolean
  error: Error | null
  copied: boolean
}

/**
 * Last line of defence: an exception while rendering would otherwise unmount the
 * whole window and leave it empty. This says so in plain words and offers a way
 * back (the downloads themselves run in the app and are not affected).
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  override state: State = { failed: false, error: null, copied: false }

  static getDerivedStateFromError(thrown: unknown): State {
    const error = thrown instanceof Error ? thrown : new Error(String(thrown))
    return { failed: true, error, copied: false }
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    // For the developer's console and the diagnostics; the person sees the plain text.
    console.error('render failed:', error, info.componentStack)
  }

  private async copy() {
    const e = this.state.error
    const text = `${e?.name ?? 'Error'}: ${e?.message ?? ''}\n${e?.stack ?? ''}`
    try {
      await navigator.clipboard.writeText(text)
      this.setState({ copied: true })
    } catch {
      /* no clipboard access: the details stay in the console */
    }
  }

  override render() {
    if (!this.state.failed) return this.props.children
    return (
      <div className="startup">
        <header className="startup-top">
          <Wordmark />
        </header>
        <main className="startup-main">
          <section className="startup-panel" role="alert" aria-labelledby="crash-title">
            <WarningCircle size={28} aria-hidden className="ic-danger" weight="fill" />
            <h1 id="crash-title">{t('Something went wrong.')}</h1>
            <p className="startup-body">
              {t(
                "Fuselane couldn't show this screen. Your downloads keep running in the background; reload the window to carry on.",
              )}
            </p>
            <div className="startup-actions">
              <button className="btn btn-primary" onClick={() => location.reload()}>
                <ArrowClockwise size={16} aria-hidden />
                {t('Reload')}
              </button>
              <button className="btn" onClick={() => void this.copy()}>
                <Copy size={16} aria-hidden />
                {this.state.copied ? t('Copied') : t('Copy details')}
              </button>
            </div>
          </section>
        </main>
      </div>
    )
  }
}
