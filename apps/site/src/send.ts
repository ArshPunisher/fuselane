// Fuse Send on the home page: the key part of the sample link scrambles into
// place once when seen (a key is random characters), and again on hover; the
// little trip below it only moves while on screen.
import { prefersReduced, watch, whenSeen } from './motion'

const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'

export function startSend() {
  const fig = document.querySelector<HTMLElement>('.anatomy')
  const key = fig?.querySelector<HTMLElement>('[data-scramble]')
  if (!fig || !key || prefersReduced()) return
  watch(fig, (v) => fig.toggleAttribute('data-play', v))
  const final = key.textContent ?? ''
  let busy = false
  const scramble = () => {
    if (busy) return
    busy = true
    const start = performance.now()
    const keep = 4 // "#v1." stays put
    const tick = (now: number) => {
      const t = (now - start) / 900
      const settled = Math.floor(keep + (final.length - keep) * Math.min(1, t))
      key.textContent = [...final]
        .map((ch, i) =>
          i < settled || ch === '…' ? ch : (CHARS[Math.floor(Math.random() * CHARS.length)] ?? ch),
        )
        .join('')
      if (t < 1) requestAnimationFrame(tick)
      else {
        key.textContent = final
        busy = false
      }
    }
    requestAnimationFrame(tick)
  }
  whenSeen(fig, scramble, 0.5)
  fig.querySelector('.link-bar')?.addEventListener('pointerenter', scramble)
}
