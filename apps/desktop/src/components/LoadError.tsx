import { WarningCircle } from '@phosphor-icons/react'
import type { UiError } from '../lib/types'
import { t } from '../lib/i18n'

/**
 * A read that failed, in the core's plain words, with a way to try again: shown
 * instead of an empty list or chart, which would read as "nothing here".
 */
export function LoadError({ error, onRetry }: { error: UiError; onRetry: () => void }) {
  return (
    <div className="load-error" role="alert">
      <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
      <p>
        {error.message}
        {error.hint ? <span className="muted"> {error.hint}</span> : null}
      </p>
      <button type="button" className="btn btn-sm" onClick={onRetry}>
        {t('Try again')}
      </button>
    </div>
  )
}
