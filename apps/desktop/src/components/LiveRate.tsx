import { useRef } from 'react'
import { rate } from '../lib/format'
import { useGlide } from '../lib/motion'

/** A speed that glides between updates; the unit follows the number. */
export function LiveRate({
  value,
  className = 'speed num',
}: {
  value: number
  className?: string
}) {
  const num = useRef<HTMLSpanElement>(null)
  const unit = useRef<HTMLSpanElement>(null)
  useGlide(value, (n) => {
    const r = rate(n)
    if (num.current) num.current.textContent = r.value
    if (unit.current) unit.current.textContent = r.unit
  })
  return (
    <p className={className}>
      <span ref={num} />
      <span className="unit" ref={unit} />
    </p>
  )
}
