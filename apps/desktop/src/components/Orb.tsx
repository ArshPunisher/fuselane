// Small network orb (DESIGN-SYSTEM.md §7): 12 dots spinning at the network's speed;
// offline orbs break apart and grey out. CSS only, so lists stay cheap.
import type { CSSProperties } from 'react'

export function Orb({
  lane,
  speed,
  state,
}: {
  lane: string
  speed: number
  state: 'live' | 'idle' | 'down'
}) {
  // speed 0..1 relative to the fastest network; one turn every 4 s (slow) to 0.9 s (fast).
  const turn = state === 'live' ? 4 - 3.1 * Math.max(0, Math.min(1, speed)) : 0
  const style = {
    '--lane': `var(--lane-${lane})`,
    '--turn': `${turn.toFixed(2)}s`,
  } as CSSProperties
  return (
    <span className="orb" data-state={state} style={style} aria-hidden="true">
      {Array.from({ length: 12 }, (_, i) => (
        <i key={i} style={{ '--i': i } as CSSProperties} />
      ))}
      <b />
    </span>
  )
}
