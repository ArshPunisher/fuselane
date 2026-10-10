// How it works: as each step crosses the middle of the screen, the picture
// beside it shows that step (split, spread, fuse). Scrolling back shows it
// again. Under reduced motion the picture still changes, without moving.
import { prefersReduced, watch, whenSeen } from './motion'

export function startStory() {
  const visual = document.querySelector<HTMLElement>('.story-visual')
  const steps = [...document.querySelectorAll<HTMLElement>('.story-step')]
  if (!visual || !steps.length || !('IntersectionObserver' in window)) return
  const show = (n: string) => {
    visual.dataset.step = n
    steps.forEach((s) =>
      s.dataset.step === n
        ? s.setAttribute('aria-current', 'step')
        : s.removeAttribute('aria-current'),
    )
  }
  // Start from the first step when the section is still below the fold.
  if (visual.getBoundingClientRect().top > innerHeight) show('1')
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries)
        if (e.isIntersecting) show((e.target as HTMLElement).dataset.step ?? '1')
    },
    { rootMargin: '-45% 0px -45% 0px' },
  )
  steps.forEach((s) => io.observe(s))
  // The falling parts only move while the picture is on screen.
  watch(visual, (v) => visual.toggleAttribute('data-play', v))
}

/** The measured numbers count up once, the first time they're seen. */
export function startProof() {
  const nums = [...document.querySelectorAll<HTMLElement>('[data-count]')]
  const list = document.querySelector('.proof-list')
  if (!nums.length || !list || prefersReduced()) return
  if (list.getBoundingClientRect().top < innerHeight * 0.9) return
  const targets = nums.map((el) => Number(el.dataset.count))
  const decimals = nums.map((el) => (el.dataset.count?.split('.')[1] ?? '').length)
  nums.forEach((el, i) => (el.textContent = (0).toFixed(decimals[i])))
  whenSeen(list, () => {
    const start = performance.now()
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / 1400)
      const e = 1 - Math.pow(1 - t, 4)
      nums.forEach((el, i) => (el.textContent = (targets[i]! * e).toFixed(decimals[i])))
      if (t < 1) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  })
}
