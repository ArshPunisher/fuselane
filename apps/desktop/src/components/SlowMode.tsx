import { useApp } from '../lib/store'
import { rateText } from '../lib/format'
import { t } from '../lib/i18n'

/** One switch that caps the overall speed, keeping the normal limits for later. */
export function SlowToggle({ labelled = true }: { labelled?: boolean }) {
  const limits = useApp((s) => s.limits)
  const setSlow = useApp((s) => s.setSlow)
  // Clicking before the backend is connected would silently do nothing.
  const ready = useApp((s) => s.backend !== null)
  return (
    <div className="slow-toggle">
      <p>
        <span className="net-name">{t('Slow mode')}</span>
        {labelled && (
          <>
            <br />
            <span className="muted num">
              {t('{rate} max', { rate: rateText(limits.slowRate) })}
            </span>
          </>
        )}
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
