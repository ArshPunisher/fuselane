import { useId } from 'react'
import { ArrowDown, ArrowUp, Lightning, Pulse } from '@phosphor-icons/react'
import type { SpeedLive } from '../lib/types'
import { t } from '../lib/i18n'

/** Scale marks in Mbps, spaced evenly round the dial (like public speed tests). */
const MARKS = [0, 5, 10, 25, 50, 100, 250, 500, 1000]
/** The dial runs from 135° to 405° (a 270° sweep, open at the bottom). */
const START = 135
const SWEEP = 270
const R = 128
const CX = 160
const CY = 160

/** 0..1 round the dial for a speed in Mbps. */
export function dialAt(mbps: number): number {
  if (mbps <= 0) return 0
  const top = MARKS[MARKS.length - 1]!
  if (mbps >= top) return 1
  const i = MARKS.findIndex((m) => m > mbps)
  const lo = MARKS[i - 1]!
  const hi = MARKS[i]!
  return (i - 1 + (mbps - lo) / (hi - lo)) / (MARKS.length - 1)
}

function polar(deg: number, r: number): [number, number] {
  const a = (deg * Math.PI) / 180
  return [CX + r * Math.cos(a), CY + r * Math.sin(a)]
}

function arc(r: number): string {
  const [x0, y0] = polar(START, r)
  const [x1, y1] = polar(START + SWEEP, r)
  return `M ${x0} ${y0} A ${r} ${r} 0 1 1 ${x1} ${y1}`
}

/** Length of the 270° arc, for the dash trick that fills it. */
const ARC_LEN = (SWEEP / 360) * 2 * Math.PI * R

export const mbpsOf = (bps: number) => (bps * 8) / 1e6

/** A speed for the readout: one decimal under 100, whole numbers above. */
export function speedText(mbps: number): string {
  return mbps >= 100 ? String(Math.round(mbps)) : mbps.toFixed(1)
}

const STEP_ICON = {
  ping: Pulse,
  down: ArrowDown,
  up: ArrowUp,
  together: Lightning,
} as const

/**
 * The speedometer: a colour sweep from cool to warm that fills to the speed
 * right now, a needle that follows it, and the number in the middle. `children`
 * sits in the centre when nothing runs (the Start button).
 */
export function SpeedGauge({
  live,
  settled,
  settledLabel,
  children,
}: {
  live: SpeedLive | null
  /** The figure to rest on after a run (Mbps), and what it is. */
  settled: number | null
  settledLabel?: string | undefined
  /** Sits over the hub when nothing runs (the Start button). */
  children?: React.ReactNode
}) {
  const id = useId().replace(/:/g, '')
  const running = live !== null
  const mbps = running ? mbpsOf(live.bps) : (settled ?? 0)
  const at = dialAt(mbps)
  const needle = START + SWEEP * at
  const Icon = live ? STEP_ICON[live.step] : settledLabel ? Lightning : ArrowDown
  const stepLabel = !live
    ? (settledLabel ?? t('Download'))
    : live.step === 'ping'
      ? t('Ping')
      : live.step === 'down'
        ? t('Download')
        : live.step === 'up'
          ? t('Upload')
          : t('Every network together')
  const idle = !running && children !== undefined
  return (
    <div
      className="gauge"
      data-running={running || undefined}
      data-idle={idle || undefined}
      data-step={live?.step ?? (settled !== null ? 'done' : undefined)}
    >
      <svg viewBox="0 0 320 300" className="gauge-svg" aria-hidden>
        <defs>
          <linearGradient id={`sweep-${id}`} x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stopColor="var(--lane-mint)" />
            <stop offset="22%" stopColor="var(--lane-tide)" />
            <stop offset="44%" stopColor="var(--lane-sky)" />
            <stop offset="64%" stopColor="var(--lane-iris)" />
            <stop offset="82%" stopColor="var(--lane-rose)" />
            <stop offset="100%" stopColor="var(--fuse)" />
          </linearGradient>
          <filter id={`glow-${id}`} x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation="6" />
          </filter>
        </defs>
        <path className="gauge-track" d={arc(R)} />
        <path className="gauge-ghost" d={arc(R)} stroke={`url(#sweep-${id})`} />
        {/* Soft glow under the filled part, then the fill itself. */}
        <path
          className="gauge-glow"
          d={arc(R)}
          stroke={`url(#sweep-${id})`}
          filter={`url(#glow-${id})`}
          strokeDasharray={ARC_LEN}
          strokeDashoffset={ARC_LEN * (1 - at)}
        />
        <path
          className="gauge-fill"
          d={arc(R)}
          stroke={`url(#sweep-${id})`}
          strokeDasharray={ARC_LEN}
          strokeDashoffset={ARC_LEN * (1 - at)}
        />
        {MARKS.map((m, i) => {
          const deg = START + (SWEEP * i) / (MARKS.length - 1)
          const [x0, y0] = polar(deg, R - 22)
          const [x1, y1] = polar(deg, R - 14)
          const [lx, ly] = polar(deg, R - 38)
          return (
            <g key={m} className="gauge-mark" data-on={mbps >= m && running ? '' : undefined}>
              <line x1={x0} y1={y0} x2={x1} y2={y1} />
              <text x={lx} y={ly} textAnchor="middle" dominantBaseline="middle">
                {m}
              </text>
            </g>
          )
        })}
        <g className="gauge-needle" style={{ transform: `rotate(${needle}deg)` }}>
          <path d={`M ${CX + 18} ${CY - 4} L ${CX + R - 30} ${CY} L ${CX + 18} ${CY + 4} Z`} />
          <circle cx={CX + R - 30} cy={CY} r="3.5" className="gauge-tip" />
        </g>
        <circle cx={CX} cy={CY} r="13" className="gauge-hub" />
      </svg>
      {idle && <div className="gauge-middle">{children}</div>}
      {(running || settled !== null) && (
        <div className="gauge-readout">
          <span className="gauge-step">
            <Icon size={14} weight="bold" aria-hidden /> {stepLabel}
          </span>
          {live?.step === 'ping' ? (
            <span className="gauge-wait" aria-label={t('Finding the speed server')}>
              <span />
              <span />
              <span />
            </span>
          ) : (
            <span className="gauge-value num">{speedText(mbps)}</span>
          )}
          <span className="gauge-unit">Mbps</span>
        </div>
      )}
    </div>
  )
}

/** This step's speed as a filled line that grows left to right. */
export function SpeedTrace({
  trace,
  step,
  progress,
}: {
  trace: number[]
  step: SpeedLive['step'] | null
  /** 0..1 through the step, so the line reaches the right edge as it ends. */
  progress: number
}) {
  const id = useId().replace(/:/g, '')
  const w = 600
  const h = 72
  const peak = Math.max(1, ...trace)
  const n = Math.max(trace.length, 2)
  const span = Math.max(n, Math.round(n / Math.max(progress, 0.05)))
  const pts = trace.map((v, i) => [(i / (span - 1)) * w, h - 4 - (v / peak) * (h - 12)] as const)
  const line = pts.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)} ${y.toFixed(1)}`).join(' ')
  const last = pts[pts.length - 1]
  const area = last ? `${line} L${last[0].toFixed(1)} ${h} L0 ${h} Z` : ''
  return (
    <svg
      className="speed-trace"
      data-step={step ?? undefined}
      viewBox={`0 0 ${w} ${h}`}
      preserveAspectRatio="none"
      aria-hidden
    >
      <defs>
        <linearGradient id={`trace-${id}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.35" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      {area && <path d={area} fill={`url(#trace-${id})`} />}
      {line && <path d={line} className="speed-trace-line" />}
    </svg>
  )
}
