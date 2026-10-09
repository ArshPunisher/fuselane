import { rate } from '../lib/format'
import type { Lane } from '../lib/lanes'

/**
 * The networks that make up the big speed, with their own speeds. The big number
 * is their sum (B8.1), so "14.2" sits over "5.4 + 2.4 + 6.4" and nothing looks off.
 */
export function SpeedSplit({ parts }: { parts: { name: string; lane: Lane; rate: number }[] }) {
  const moving = parts.filter((p) => p.rate > 0)
  if (moving.length < 2) return null
  return (
    <ul className="speed-split" aria-label="Speed by network">
      {moving.map((p) => {
        const r = rate(p.rate)
        return (
          <li key={p.name}>
            <i style={{ background: `var(--lane-${p.lane})` }} aria-hidden />
            {p.name}{' '}
            <span className="num">
              {r.value}
              <span className="unit"> {r.unit}</span>
            </span>
          </li>
        )
      })}
      <li className="eq">adds up to the speed above</li>
    </ul>
  )
}
