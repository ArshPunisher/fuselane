import { useEffect } from 'react'
import { WarningCircle, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { t } from '../lib/i18n'

export function Toast() {
  const toast = useApp((s) => s.toast)
  const dismiss = useApp((s) => s.dismissToast)
  useEffect(() => {
    if (!toast) return
    const t = setTimeout(dismiss, 8000)
    return () => clearTimeout(t)
  }, [toast, dismiss])
  if (!toast) return null
  return (
    <div className="toast" role="alert" key={toast.at}>
      <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
      <p>
        {toast.message}
        {toast.hint ? <span className="muted"> {toast.hint}</span> : null}
      </p>
      <button className="icon-btn" aria-label={t('Dismiss')} onClick={dismiss}>
        <X size={16} aria-hidden />
      </button>
    </div>
  )
}
