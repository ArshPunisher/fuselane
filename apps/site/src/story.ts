// How it works: on wide screens, as each step crosses the middle of the screen, the picture
// beside it shows that step (split, spread, fuse); on phones the card plays the steps by
// itself. Under reduced motion the picture still changes, without moving.
import { prefersReduced, whenSeen } from './motion'

export function startStory() {
  const visual = document.querySelector<HTMLElement>('.story-visual')
  const steps = [...document.querySelectorAll<HTMLElement>('.story-step')]
  const slot = visual?.querySelector<HTMLElement>('.sv-text')
  if (!visual || !steps.length || !('IntersectionObserver' in window)) return
  const reduced = prefersReduced()
  const show = (n: string) => {
    const changed = visual.dataset.step !== n
    visual.dataset.step = n
    steps.forEach((s) =>
      s.dataset.step === n
        ? s.setAttribute('aria-current', 'step')
        : s.removeAttribute('aria-current'),
    )
    // On phones the card carries the current step's words (the card is hidden
    // from screen readers, which read the steps themselves).
    const step = steps.find((s) => s.dataset.step === n)
    if (!slot || !step || (!changed && slot.childElementCount)) return
    slot.replaceChildren(...[...step.children].map((c) => c.cloneNode(true)))
    if (!reduced && changed)
      slot.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 280, easing: 'ease-out' })
  }
  // Phones play the steps by themselves; wider screens follow the scroll. The
  // mode follows the width, so turning a tablet or resizing the window works.
  const phone = matchMedia('(max-width: 959px)')
  let stop = () => {}
  const start = () => {
    stop()
    stop = phone.matches
      ? playOnPhones(visual, steps, show, reduced)
      : followScroll(visual, steps, show)
  }
  phone.addEventListener('change', start)
  start()
}

/** Wide screens: as each step crosses the middle, the card shows it. */
function followScroll(
  visual: HTMLElement,
  steps: HTMLElement[],
  show: (n: string) => void,
): () => void {
  // Start from the first step when the section is still below the fold.
  show(visual.getBoundingClientRect().top > innerHeight ? '1' : (visual.dataset.step ?? '1'))
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries)
        if (e.isIntersecting) show((e.target as HTMLElement).dataset.step ?? '1')
    },
    { rootMargin: '-45% 0px -45% 0px' },
  )
  steps.forEach((s) => io.observe(s))
  // The falling parts only move while the picture is on screen.
  const seen = new IntersectionObserver((e) =>
    visual.toggleAttribute(
      'data-play',
      e.some((x) => x.isIntersecting),
    ),
  )
  seen.observe(visual)
  return () => {
    io.disconnect()
    seen.disconnect()
  }
}

/** Seconds each step shows before the next, on phones. */
const STEP_MS = 3600

/** Phones: the card plays the three steps while on screen, with tabs to pick one. */
function playOnPhones(
  visual: HTMLElement,
  steps: HTMLElement[],
  show: (n: string) => void,
  reduced: boolean,
): () => void {
  const story = visual.closest('.story')
  story?.setAttribute('data-autoplay', '')
  const tabs = document.createElement('div')
  tabs.className = 'sv-tabs'
  tabs.setAttribute('role', 'tablist')
  tabs.setAttribute('aria-label', 'Steps')
  const buttons = steps.map((step) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.setAttribute('role', 'tab')
    b.dataset.step = step.dataset.step ?? '1'
    const name = document.createElement('span')
    name.textContent = step.querySelector('h3')?.textContent ?? ''
    const bar = document.createElement('i')
    bar.setAttribute('aria-hidden', 'true')
    b.append(name, bar)
    tabs.append(b)
    return b
  })
  visual.after(tabs)
  let at = 0
  let timer = 0
  let onScreen = false
  const go = (i: number) => {
    at = (i + steps.length) % steps.length
    show(steps[at]!.dataset.step ?? '1')
    buttons.forEach((b, k) => {
      b.setAttribute('aria-selected', String(k === at))
      // Restart the fill on the current tab.
      b.classList.remove('run')
    })
    if (!reduced && onScreen) {
      void buttons[at]!.offsetWidth
      buttons[at]!.classList.add('run')
    }
    clearTimeout(timer)
    if (!reduced && onScreen) timer = window.setTimeout(() => go(at + 1), STEP_MS)
  }
  buttons.forEach((b, k) => b.addEventListener('click', () => go(k)))
  // Plays while any of the card is on screen.
  const seen = new IntersectionObserver((entries) => {
    const v = entries.some((e) => e.isIntersecting)
    if (v === onScreen) return
    onScreen = v
    visual.toggleAttribute('data-play', v)
    if (v) go(at)
    else clearTimeout(timer)
  })
  seen.observe(visual)
  go(0)
  return () => {
    seen.disconnect()
    clearTimeout(timer)
    tabs.remove()
    story?.removeAttribute('data-autoplay')
  }
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
