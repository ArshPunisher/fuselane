// Download page: the visitor's system picked, links to the exact files of the
// current version (from the signed update feed), and each file's size.
import { REPO, detectOs, fileUrl, latestVersion, type Os } from './common'

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
  const version = await latestVersion('../')
  document.querySelectorAll<HTMLAnchorElement>('a[data-file]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.file ?? '') : `${REPO}/releases`
  })
  document.querySelectorAll<HTMLAnchorElement>('a[data-cli]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.cli ?? '', true) : `${REPO}/releases`
  })
  if (!version) return
  $('#version-tag')!.textContent = version
  $<HTMLAnchorElement>('#sums')!.href = `${REPO}/releases/download/v${version}/SHA256SUMS`
  $<HTMLAnchorElement>('#notes')!.href = `${REPO}/releases/tag/v${version}`
  void sizes(version)
}

const os = detectOs(navigator.userAgent, navigator.platform)
tabs(os === 'other' ? 'mac' : os)
void links()
