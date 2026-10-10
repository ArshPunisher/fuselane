// The race: Wi-Fi alone against Wi-Fi and Ethernet together, at the speeds a
// real network check measured on one Mac (Wi-Fi 117 Mbps, Ethernet 113 Mbps).
// The times are worked out from those speeds and say so on the page. The bars
// grow in step with the bytes (linear, as progress should be); the slower lane
// always takes about four seconds, whatever the size, and the clock shows the
// time it stands for.
import { duration, loop, prefersReduced, whenSeen } from './motion'

const REAL_SECONDS = 4.2

/** Seconds to move `gb` gigabytes (decimal, as network speeds are) at `mbps`. */
export const seconds = (gb: number, mbps: number) => (gb * 8000) / mbps

/** "11:24", or "1:02:07" past an hour. */
export function clock(s: number) {
  const t = Math.max(0, Math.round(s))
  const h = Math.floor(t / 3600)
  const m = Math.floor((t % 3600) / 60)
  const sec = String(t % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

export function startRace() {
  const race = document.querySelector<HTMLElement>('.race')
  if (!race) return
  const lanes = [...race.querySelectorAll<HTMLElement>('.lane')]
  const tracks = lanes.map((l) => l.querySelector<HTMLElement>('.track')!)
  const times = lanes.map((l) => l.querySelector<HTMLOutputElement>('.lane-time')!)
  const speeds = lanes.map((l) => Number(l.dataset.mbps))
  const clockEl = race.querySelector<HTMLElement>('#race-clock')
  const result = race.querySelector<HTMLElement>('#race-result')
  const runBtn = race.querySelector<HTMLButtonElement>('#race-run')
  const radios = [...race.querySelectorAll<HTMLInputElement>('input[name="race-size"]')]
  const reduced = prefersReduced()
  let size = Number(radios.find((r) => r.checked)?.value ?? 10)
  let t = 0
  let running = false

  const totals = () => speeds.map((mbps) => seconds(size, mbps))
  const slowest = () => Math.max(...totals())

  const draw = () => {
    const all = totals()
    const scale = slowest() / REAL_SECONDS
    const sim = Math.min(t * scale, slowest())
    all.forEach((total, i) => {
      const p = Math.min(1, sim / total)
      tracks[i]!.style.setProperty('--p', p.toFixed(4))
      const done = p >= 1
      if (done !== tracks[i]!.hasAttribute('data-done')) {
        tracks[i]!.toggleAttribute('data-done', done)
        times[i]!.toggleAttribute('data-wait', !done)
      }
    })
    if (clockEl) clockEl.textContent = clock(sim)
  }

  const finish = () => {
    const [one, both] = totals() as [number, number]
    times[0]!.textContent = duration(one)
    times[1]!.textContent = duration(both)
    if (result)
      result.textContent = `Both together: ${duration(both)}, which is ${duration(one - both)} sooner.`
  }

  const run = () => {
    finish()
    if (reduced) {
      t = REAL_SECONDS
      draw()
      return
    }
    t = -0.35
    running = true
    race.setAttribute('data-running', '')
    times.forEach((el) => el.setAttribute('data-wait', ''))
    tracks.forEach((el) => el.removeAttribute('data-done'))
    draw()
  }

  loop(race, (dt) => {
    if (!running) return
    t += dt
    draw()
    if (t >= REAL_SECONDS) {
      running = false
      race.removeAttribute('data-running')
    }
  })

  radios.forEach((r) =>
    r.addEventListener('change', () => {
      size = Number(r.value)
      run()
    }),
  )
  runBtn?.addEventListener('click', run)

  finish()
  if (reduced || race.getBoundingClientRect().top < innerHeight * 0.6) {
    t = REAL_SECONDS
    draw()
    return
  }
  // Ready at the start line until it's seen.
  t = 0
  race.setAttribute('data-running', '')
  times.forEach((el) => el.setAttribute('data-wait', ''))
  draw()
  whenSeen(race, run, 0.5)
}
