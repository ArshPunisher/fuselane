import { useApp } from '../lib/store'
import { rateText } from '../lib/format'

/** One switch that caps the overall speed, keeping the normal limits for later. */
export function SlowToggle({ labelled = true }: { labelled?: boolean }) {
  const limits = useApp((s) => s.limits)
  const setSlow = useApp((s) => s.setSlow)
  return (
    <div className="slow-toggle">
      <p>
        <span className="net-name">Slow mode</span>
        {labelled && (
          <>
            <br />
            <span className="muted num">{rateText(limits.slowRate)} max</span>
          </>
        )}
      </p>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={limits.slow}
        aria-label="Slow mode"
        onClick={() => void setSlow(!limits.slow)}
      />
    </div>
  )
}
