// Built in: tabs for the four groups of tools (arrow keys, Home and End), and
// the small-things marquee, which only moves while on screen and never under
// reduced motion (then the chips simply wrap).
import { prefersReduced, watch } from './motion'

export function tabs(list: HTMLElement) {
  const all = [...list.querySelectorAll<HTMLButtonElement>('[role="tab"]')]
  const select = (tab: HTMLButtonElement, focus = false) => {
    for (const t of all) {
      const on = t === tab
      t.setAttribute('aria-selected', String(on))
      t.tabIndex = on ? 0 : -1
      const panel = document.getElementById(t.getAttribute('aria-controls') ?? '')
      if (panel) panel.hidden = !on
    }
    if (focus) tab.focus()
    tab.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }
  all.forEach((t, i) => {
    t.addEventListener('click', () => select(t))
    t.addEventListener('keydown', (e) => {
      const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
      if (e.key === 'Home') select(all[0]!, true)
      else if (e.key === 'End') select(all[all.length - 1]!, true)
      else if (step) select(all[(i + step + all.length) % all.length]!, true)
      else return
      e.preventDefault()
    })
  })
  const first = all.find((t) => t.getAttribute('aria-selected') === 'true') ?? all[0]
  if (first)
    for (const t of all) {
      const panel = document.getElementById(t.getAttribute('aria-controls') ?? '')
      if (panel) panel.hidden = t !== first
    }
}

export function startTools() {
  const list = document.querySelector<HTMLElement>('.tools-tabs')
  if (list) tabs(list)
  const marquee = document.querySelector<HTMLElement>('.marquee')
  const track = marquee?.querySelector<HTMLElement>('.marquee-track')
  const items = track?.querySelector('.marquee-list')
  if (!marquee || !track || !items || prefersReduced()) return
  // A second copy, hidden from screen readers, makes the loop seamless.
  const copy = items.cloneNode(true) as HTMLElement
  copy.setAttribute('aria-hidden', 'true')
  track.append(copy)
  watch(marquee, (v) => marquee.toggleAttribute('data-play', v))
}
