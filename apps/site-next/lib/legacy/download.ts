// Download page: the visitor's system picked, links to the exact files of the
// current version (from the signed update feed), and each file's size.
import { REPO, detectOs, fileUrl, isStill, latestVersion, type Os } from './common'
import { pageSignal, prefersReduced, watch, whenSeen } from './motion'

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)

function tabs(initial: Os) {
  const list = [...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')]
  const select = (tab: HTMLButtonElement, focus = false) => {
    for (const t of list) {
      const on = t === tab
      t.setAttribute('aria-selected', String(on))
      t.tabIndex = on ? 0 : -1
      const panel = document.getElementById(t.getAttribute('aria-controls') ?? '')
      if (panel) panel.hidden = !on
    }
    if (focus) tab.focus()
  }
  list.forEach((t, i) => {
    t.addEventListener('click', () => select(t))
    t.addEventListener('keydown', (e) => {
      const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
      if (e.key === 'Home') select(list[0]!, true)
      else if (e.key === 'End') select(list[list.length - 1]!, true)
      else if (step) select(list[(i + step + list.length) % list.length]!, true)
      else return
      e.preventDefault()
    })
  })
  const mine = list.find((t) => t.dataset.os === initial)
  if (mine) {
    select(mine)
    const badge = document.createElement('span')
    badge.className = 'detected'
    badge.textContent = 'Yours'
    mine.append(badge)
  }
  // A link into one system's panel (download/#open-anyway) opens that panel.
  const follow = () => {
    const id = decodeURIComponent(location.hash.slice(1))
    const target = id ? document.getElementById(id) : null
    const panel = target?.closest<HTMLElement>('[role="tabpanel"]')
    const tab = panel && list.find((t) => t.getAttribute('aria-controls') === panel.id)
    if (!target || !tab) return
    select(tab)
    target.scrollIntoView()
  }
  follow()
  addEventListener('hashchange', follow, { signal: pageSignal() })
}

async function sizes(version: string) {
  try {
    const r = await fetch(
      `https://api.github.com/repos/ArshPunisher/fuselane/releases/tags/v${version}`,
    )
    if (!r.ok) return
    const rel = (await r.json()) as { assets?: { name: string; size: number }[] }
    const mb = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
    document.querySelectorAll<HTMLElement>('[data-size]').forEach((el) => {
      const a = rel.assets?.find((x) => x.name === `Fuselane_${version}_${el.dataset.size}`)
      if (a) el.textContent = `${mb.format(a.size / 1024 / 1024)} MB`
    })
  } catch {
    /* sizes are a nicety */
  }
}

async function links() {
  const version = await latestVersion()
  document.querySelectorAll<HTMLAnchorElement>('a[data-file]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.file ?? '') : `${REPO}/releases`
  })
  document.querySelectorAll<HTMLAnchorElement>('a[data-cli]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.cli ?? '', true) : `${REPO}/releases`
  })
  if (!version) return
  $('#version-tag')!.textContent = version
  document
    .querySelectorAll<HTMLElement>('[data-ver-text]')
    .forEach((el) => (el.textContent = version))
  $('[data-ver]')!.hidden = false
  $('[data-nover]')!.hidden = true
  $<HTMLAnchorElement>('#sums')!.href = `${REPO}/releases/download/v${version}/SHA256SUMS`
  $<HTMLAnchorElement>('#notes')!.href = `${REPO}/releases/tag/v${version}`
  void sizes(version)
}

/**
 * The illustrated steps take turns while on screen: the pointer travels to the
 * button to press and clicks it. Pointing at a step (or focusing into it)
 * shows that one and holds the turn for a while. Still under reduced motion.
 */
function guides() {
  if (prefersReduced()) return
  document.querySelectorAll<HTMLElement>('.guide-steps').forEach((list) => {
    const steps = [...list.querySelectorAll<HTMLElement>('.guide-step')]
    let active = 0
    let visible = false
    let holdUntil = 0
    const show = (i: number) => {
      active = i
      steps.forEach((s, k) => s.toggleAttribute('data-active', k === i))
    }
    watch(list, (v) => {
      visible = v
      list.toggleAttribute('data-play', v)
      if (v && !steps.some((s) => s.hasAttribute('data-active'))) show(0)
    })
    const every = window.setInterval(() => {
      if (!visible || document.hidden || isStill() || Date.now() < holdUntil) return
      show((active + 1) % steps.length)
    }, 3400)
    pageSignal()?.addEventListener('abort', () => clearInterval(every), { once: true })
    steps.forEach((s, i) => {
      const pick = () => {
        holdUntil = Date.now() + 8000
        if (active !== i) show(i)
      }
      s.addEventListener('pointerenter', pick)
      s.addEventListener('focusin', pick)
    })
  })
  // The install script's output appears line by line, once.
  const term = document.querySelector<HTMLElement>('.term')
  if (term) whenSeen(term, () => term.setAttribute('data-play', ''), 0.6)
}

export function initDownload() {
  const os = detectOs(navigator.userAgent, navigator.platform)
  tabs(os === 'other' ? 'mac' : os)
  void links()
  guides()
}
