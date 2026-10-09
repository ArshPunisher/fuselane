// Shared by every page: icons (Phosphor's own SVGs), the live star count, the
// current page in the nav, reveal-on-scroll, the pointer spotlight, copy buttons.
import './site.css'
import star from '@phosphor-icons/core/assets/regular/star.svg?raw'
import github from '@phosphor-icons/core/assets/regular/github-logo.svg?raw'
import download from '@phosphor-icons/core/assets/regular/download-simple.svg?raw'
import arrowUpRight from '@phosphor-icons/core/assets/regular/arrow-up-right.svg?raw'
import arrowRight from '@phosphor-icons/core/assets/regular/arrow-right.svg?raw'
import apple from '@phosphor-icons/core/assets/regular/apple-logo.svg?raw'
import windows from '@phosphor-icons/core/assets/regular/windows-logo.svg?raw'
import linux from '@phosphor-icons/core/assets/regular/linux-logo.svg?raw'
import terminal from '@phosphor-icons/core/assets/regular/terminal-window.svg?raw'
import share from '@phosphor-icons/core/assets/regular/share-network.svg?raw'
import bug from '@phosphor-icons/core/assets/regular/bug.svg?raw'
import bulb from '@phosphor-icons/core/assets/regular/lightbulb.svg?raw'
import code from '@phosphor-icons/core/assets/regular/code.svg?raw'
import notes from '@phosphor-icons/core/assets/regular/notepad.svg?raw'
import moon from '@phosphor-icons/core/assets/regular/moon.svg?raw'
import calendar from '@phosphor-icons/core/assets/regular/calendar-dots.svg?raw'
import power from '@phosphor-icons/core/assets/regular/power.svg?raw'
import gauge from '@phosphor-icons/core/assets/regular/gauge.svg?raw'
import refresh from '@phosphor-icons/core/assets/regular/arrows-clockwise.svg?raw'
import shield from '@phosphor-icons/core/assets/regular/shield-check.svg?raw'
import puzzle from '@phosphor-icons/core/assets/regular/puzzle-piece.svg?raw'
import magnet from '@phosphor-icons/core/assets/regular/magnet.svg?raw'
import list from '@phosphor-icons/core/assets/regular/list-numbers.svg?raw'
import plug from '@phosphor-icons/core/assets/regular/plugs-connected.svg?raw'
import copy from '@phosphor-icons/core/assets/regular/copy.svg?raw'

export const REPO = 'https://github.com/ArshPunisher/fuselane'

const ICONS: Record<string, string> = {
  star,
  github,
  download,
  'arrow-up-right': arrowUpRight,
  'arrow-right': arrowRight,
  apple,
  windows,
  linux,
  terminal,
  share,
  bug,
  bulb,
  code,
  notes,
  moon,
  calendar,
  power,
  gauge,
  refresh,
  shield,
  puzzle,
  magnet,
  list,
  plug,
  copy,
}

/** `<i data-icon="star"></i>` becomes the icon, hidden from screen readers. */
function icons() {
  document.querySelectorAll<HTMLElement>('[data-icon]').forEach((el) => {
    const svg = ICONS[el.dataset.icon ?? '']
    if (!svg) return
    el.outerHTML = svg.replace('<svg ', '<svg aria-hidden="true" focusable="false" ')
  })
}

/** The repo's star count, cached for the visit; nothing shows if GitHub can't be reached. */
async function stars() {
  const slots = document.querySelectorAll<HTMLElement>('[data-stars]')
  if (!slots.length) return
  let n: number | null = null
  try {
    const cached = sessionStorage.getItem('fuselane.stars')
    if (cached) n = Number(cached)
  } catch {
    /* storage blocked */
  }
  if (n === null) {
    try {
      const r = await fetch('https://api.github.com/repos/ArshPunisher/fuselane', {
        headers: { Accept: 'application/vnd.github+json' },
      })
      if (r.ok) n = ((await r.json()) as { stargazers_count?: number }).stargazers_count ?? null
      if (n !== null) sessionStorage.setItem('fuselane.stars', String(n))
    } catch {
      /* offline or rate-limited: no count */
    }
  }
  // "0" next to Star reads worse than no number at all.
  if (n === null || n < 1) return
  const text = new Intl.NumberFormat(undefined, { notation: 'compact' }).format(n)
  slots.forEach((s) => (s.textContent = text))
}

function currentPage() {
  const here = location.pathname.replace(/index\.html$/, '').replace(/\/$/, '')
  document.querySelectorAll<HTMLAnchorElement>('.nav-links a, .nav-mobile a').forEach((a) => {
    const to = new URL(a.href).pathname.replace(/index\.html$/, '').replace(/\/$/, '')
    if (to === here && !a.hash) a.setAttribute('aria-current', 'page')
  })
}

function reveal() {
  const els = document.querySelectorAll<HTMLElement>('.reveal, .frame')
  if (!('IntersectionObserver' in window)) {
    els.forEach((e) => e.classList.add('seen'))
    return
  }
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          e.target.classList.add('seen')
          io.unobserve(e.target)
        }
      }
    },
    { rootMargin: '0px 0px -8% 0px', threshold: 0.12 },
  )
  els.forEach((e) => io.observe(e))
}

/** Panels with .spot light up where the pointer is (CSS reads --mx/--my). */
function spotlight() {
  if (matchMedia('(hover: none)').matches) return
  document.querySelectorAll<HTMLElement>('.spot').forEach((el) => {
    el.addEventListener('pointermove', (e) => {
      const r = el.getBoundingClientRect()
      el.style.setProperty('--mx', `${e.clientX - r.left}px`)
      el.style.setProperty('--my', `${e.clientY - r.top}px`)
    })
  })
}

export function toast(text: string) {
  let t = document.querySelector<HTMLElement>('.toast')
  if (!t) {
    t = document.createElement('p')
    t.className = 'toast'
    t.setAttribute('role', 'status')
    document.body.append(t)
  }
  t.textContent = text
  window.setTimeout(() => {
    if (t) t.textContent = ''
  }, 2400)
}

function copyButtons() {
  document.querySelectorAll<HTMLButtonElement>('button[data-copy]').forEach((b) => {
    b.addEventListener('click', async () => {
      const text = document.getElementById(b.dataset.copy ?? '')?.textContent?.trim() ?? ''
      try {
        await navigator.clipboard.writeText(text)
        b.textContent = 'Copied'
      } catch {
        b.textContent = 'Select and copy'
      }
      window.setTimeout(() => (b.textContent = 'Copy'), 2000)
    })
  })
}

export type Os = 'mac' | 'windows' | 'linux' | 'other'

export function detectOs(ua: string, platform: string): Os {
  const s = `${platform} ${ua}`.toLowerCase()
  if (/iphone|ipad|android/.test(s)) return 'other'
  if (/mac/.test(s)) return 'mac'
  if (/win/.test(s)) return 'windows'
  if (/linux|x11|cros/.test(s)) return 'linux'
  return 'other'
}

/** The version in the signed update feed, or null (offline, not deployed yet). */
export async function latestVersion(base: string): Promise<string | null> {
  try {
    const r = await fetch(`${base}updates/latest.json`, { cache: 'no-cache' })
    if (r.ok) return ((await r.json()) as { version?: string }).version ?? null
  } catch {
    /* fall back to the releases page */
  }
  return null
}

export function fileUrl(version: string, suffix: string, cli = false) {
  const name = cli ? `fuselane-cli_${version}_${suffix}` : `Fuselane_${version}_${suffix}`
  return `${REPO}/releases/download/v${version}/${name}`
}

icons()
currentPage()
reveal()
spotlight()
copyButtons()
void stars()
