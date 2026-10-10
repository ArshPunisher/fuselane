import { useApp } from '../lib/store'
import { rateText } from '../lib/format'
import { t } from '../lib/i18n'

/** One switch that caps the overall speed, keeping the normal limits for later. */
export function SlowToggle({ labelled = true }: { labelled?: boolean }) {
  const limits = useApp((s) => s.limits)
  const setSlow = useApp((s) => s.setSlow)
  // Clicking before the backend is connected would silently do nothing.
  const ready = useApp((s) => s.backend !== null)
  // In Settings the row already names it: just the switch.
  if (!labelled)
    return (
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={limits.slow}
        aria-label={t('Slow mode')}
        disabled={!ready}
        onClick={() => void setSlow(!limits.slow)}
      />
    )
  return (
    <div className="slow-toggle">
      <p>
        <span className="net-name">{t('Slow mode')}</span>
        <br />
        <span className="muted num">{t('{rate} max', { rate: rateText(limits.slowRate) })}</span>
      </p>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={limits.slow}
        aria-label={t('Slow mode')}
        disabled={!ready}
        onClick={() => void setSlow(!limits.slow)}
      />
    </div>
  )
}
