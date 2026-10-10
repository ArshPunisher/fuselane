// Shared by every page: icons (Phosphor's own SVGs), the live star count, the
// navbar, reveal-on-scroll, the pointer light on panels, copy buttons.
import { MENU_ICONS } from '@/components/menu-icons'
import { pageSignal } from './motion'

/** Each element is wired once, even when the page script runs again. */
const once = (el: Element) => {
  if (el.hasAttribute('data-bound')) return false
  el.setAttribute('data-bound', '')
  return true
}

export const REPO = 'https://github.com/ArshPunisher/fuselane'

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
  const label = new Intl.NumberFormat(undefined, { notation: 'compact' }).format(n)
  slots.forEach((s) => (s.textContent = label))
}

/**
 * The navbar: a Fuse underline glides to whichever link the pointer or focus is
 * on (and rests on the current page), the bar turns to glass once the page
 * scrolls, and on phones a menu button opens the page list.
 */
function navbar() {
  const nav = document.querySelector<HTMLElement>('[data-nav]')
  if (!nav) return
  const track = nav.querySelector<HTMLElement>('.nav-track')
  const links = [...nav.querySelectorAll<HTMLAnchorElement>('.nav-links a')]
  const glideTo = (a: HTMLElement | undefined) => {
    if (!track) return
    if (!a) {
      track.style.setProperty('--glide-o', '0')
      return
    }
    track.style.setProperty('--glide-x', `${a.offsetLeft}px`)
    track.style.setProperty('--glide-w', `${a.offsetWidth}px`)
    track.style.setProperty('--glide-s', String(Math.max(0, a.offsetWidth - 26)))
    track.style.setProperty('--glide-o', '1')
  }
  const rest = () => glideTo(links.find((a) => a.getAttribute('aria-current') === 'page'))
  links.forEach((a) => {
    a.addEventListener('pointerenter', () => glideTo(a))
    a.addEventListener('focus', () => glideTo(a))
  })
  track?.addEventListener('pointerleave', rest)
  track?.addEventListener('focusout', (e) => {
    if (!track.contains(e.relatedTarget as Node)) rest()
  })
  rest()
  // The web font changes link widths once it arrives.
  void document.fonts?.ready.then(rest)
  // Scrolled: a sentinel at the top of the page leaves view (no scroll listener).
  const sentinel = document.querySelector('.nav-sentinel')
  if (sentinel && 'IntersectionObserver' in window) {
    new IntersectionObserver(([e]) => {
      nav.toggleAttribute('data-scrolled', !e?.isIntersecting)
    }).observe(sentinel)
  }
  // Phones: the menu button opens the page list.
  const button = nav.querySelector<HTMLButtonElement>('.nav-menu')
  const sheet = document.querySelector<HTMLElement>('#nav-sheet')
  if (!button || !sheet) return
  const set = (open: boolean) => {
    button.setAttribute('aria-expanded', String(open))
    sheet.hidden = !open
    nav.toggleAttribute('data-open', open)
    // Swap the icon's drawing in place (React owns the <svg> element itself).
    const icon = button.querySelector('svg')
    if (icon) icon.innerHTML = MENU_ICONS[open ? 'close' : 'menu']
  }
  button.addEventListener('click', () => set(sheet.hidden))
  sheet.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('a')) set(false)
  })
  addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !sheet.hidden) {
      set(false)
      button.focus()
    }
  })
  matchMedia('(min-width: 960px)').addEventListener('change', (m) => m.matches && set(false))
  // A page change (no reload): close the sheet and rest the underline on the new page.
  addEventListener('fuselane:route', () => {
    set(false)
    rest()
  })
}

/**
 * Reveal on scroll. Only what starts below the fold is held back, so the first
 * screen never flashes, nothing is hidden without JavaScript, and nothing waits
 * under reduced motion.
 */
function reveal() {
  const els = [...document.querySelectorAll<HTMLElement>('.reveal, .foot')]
  if (!('IntersectionObserver' in window) || matchMedia('(prefers-reduced-motion: reduce)').matches)
    return
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          e.target.classList.add('seen')
          io.unobserve(e.target)
        }
      }
    },
    { rootMargin: '0px 0px -6% 0px', threshold: 0.08 },
  )
  pageSignal()?.addEventListener('abort', () => io.disconnect(), { once: true })
  for (const el of els) {
    if (el.classList.contains('seen')) continue
    // Still waiting from an earlier page (the footer stays across pages): watch it again.
    if (el.classList.contains('pending')) {
      io.observe(el)
      continue
    }
    if (el.getBoundingClientRect().top > innerHeight) {
      el.classList.add('pending')
      io.observe(el)
    }
  }
}

/** FAQ and guide: the topic list marks the topic being read. */
function topics() {
  const links = [...document.querySelectorAll<HTMLAnchorElement>('[data-toc] a')]
  if (!links.length || !('IntersectionObserver' in window)) return
  const targets = links
    .map((a) => document.getElementById(decodeURIComponent(a.hash.slice(1))))
    .filter((t): t is HTMLElement => Boolean(t))
  const mark = (id: string) =>
    links.forEach((a) => a.setAttribute('aria-current', String(a.hash === `#${id}`)))
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) if (e.isIntersecting) mark(e.target.id)
    },
    { rootMargin: '-30% 0px -60% 0px' },
  )
  targets.forEach((t) => io.observe(t))
  pageSignal()?.addEventListener('abort', () => io.disconnect(), { once: true })
  // A pick is marked at once: the last topic may never reach the middle band.
  links.forEach((l) => l.addEventListener('click', () => mark(l.hash.slice(1))))
}

/** Panels with .spot light up where the pointer is (CSS reads --mx/--my). */
function spotlight() {
  if (matchMedia('(hover: none)').matches) return
  document.querySelectorAll<HTMLElement>('.spot').forEach((el) => {
    if (!once(el)) return
    el.addEventListener('pointermove', (e) => {
      const r = el.getBoundingClientRect()
      el.style.setProperty('--mx', `${e.clientX - r.left}px`)
      el.style.setProperty('--my', `${e.clientY - r.top}px`)
    })
  })
}

/**
 * Pause animations (WCAG 2.2.2): one switch, in the footer of every page and on
 * the hero, stops everything that loops by itself. The choice is remembered on
 * this device. Under reduced motion nothing loops, so the switch isn't shown.
 */
// One pause state for the whole visit, shared by the footer's switch (which
// stays across pages) and the hero's (which comes and goes with the page).
let still: boolean | null = null

function stillToggle() {
  const root = document.documentElement
  const buttons = [...document.querySelectorAll<HTMLButtonElement>('[data-still-toggle]')]
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
    buttons.forEach((b) => (b.hidden = true))
    return
  }
  if (still === null) {
    try {
      still = localStorage.getItem('fuselane.still') === '1'
    } catch {
      still = false /* storage blocked */
    }
  }
  const apply = () => {
    root.toggleAttribute('data-still', Boolean(still))
    document.querySelectorAll<HTMLButtonElement>('[data-still-toggle]').forEach((b) => {
      b.setAttribute('aria-pressed', String(still))
      const label = still ? 'Play animations' : 'Pause animations'
      if (b.hasAttribute('aria-label')) b.setAttribute('aria-label', label)
      else b.textContent = label
    })
  }
  buttons.forEach(
    (b) =>
      once(b) &&
      b.addEventListener('click', () => {
        still = !still
        try {
          localStorage.setItem('fuselane.still', still ? '1' : '0')
        } catch {
          /* storage blocked: it still works for this visit */
        }
        apply()
      }),
  )
  apply()
}

/** True while the visitor has paused animations. */
export const isStill = () => document.documentElement.hasAttribute('data-still')

/** Background tab: CSS animations rest too (canvas loops stop by themselves). */
function restWhenHidden() {
  const set = () => document.documentElement.toggleAttribute('data-hidden', document.hidden)
  document.addEventListener('visibilitychange', set)
  set()
}

export function toast(message: string) {
  let t = document.querySelector<HTMLElement>('.toast')
  if (!t) {
    t = document.createElement('p')
    t.className = 'toast'
    t.setAttribute('role', 'status')
    document.body.append(t)
  }
  t.textContent = message
  window.setTimeout(() => {
    if (t) t.textContent = ''
  }, 2400)
}

/** Says what just happened to screen readers, without showing anything. */
export function announce(message: string) {
  let r = document.querySelector<HTMLElement>('#sr-status')
  if (!r) {
    r = document.createElement('p')
    r.id = 'sr-status'
    r.className = 'sr-only'
    r.setAttribute('role', 'status')
    document.body.append(r)
  }
  r.textContent = message
}

function copyButtons() {
  document.querySelectorAll<HTMLButtonElement>('button[data-copy]').forEach((b) => {
    if (!once(b)) return
    b.addEventListener('click', async () => {
      const value = document.getElementById(b.dataset.copy ?? '')?.textContent?.trim() ?? ''
      try {
        await navigator.clipboard.writeText(value.replace(/\s+/g, ' '))
        b.textContent = 'Copied'
        announce('Copied to the clipboard.')
      } catch {
        b.textContent = 'Select and copy'
        announce('Copying was blocked. Select the text and copy it.')
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

/** Once per visit: the navbar and the background-tab rest. */
export function initShell() {
  document.documentElement.classList.add('js')
  navbar()
  restWhenHidden()
}

/** Every page view (and the persistent footer, wired only once). */
export function initPage() {
  reveal()
  spotlight()
  stillToggle()
  copyButtons()
  void stars()
  topics()
}
