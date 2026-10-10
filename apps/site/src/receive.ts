// The Fuse Send landing page: hands the link to the app (fuselane://send/…) and
// says what to do without it. The token stays in this tab: it's never fetched,
// logged or sent anywhere (the referrer policy keeps the address to itself too).
import { toast } from './common'
import './css/receive.css'

/** v1 token: "v1." + base64url(20-byte info-hash, 32-byte key, 1 flag byte). */
const BODY_CHARS = Math.ceil((53 * 4) / 3)

export type TokenState = 'ok' | 'empty' | 'newer' | 'broken'

export function checkToken(fragment: string): TokenState {
  const token = fragment.replace(/^#/, '').trim()
  if (!token) return 'empty'
  const m = /^v(\d+)\.([A-Za-z0-9_-]*)$/.exec(token)
  if (!m) return 'broken'
  const version = Number(m[1])
  if (version > 1) return 'newer'
  if (version < 1 || m[2]!.length !== BODY_CHARS) return 'broken'
  return 'ok'
}

const COPY: Record<Exclude<TokenState, 'ok'>, { title: string; lead: string }> = {
  empty: {
    title: 'This link is missing its key',
    lead: 'The part after # didn’t come through, and the file can’t be opened without it. Ask the sender to copy the whole link again.',
  },
  broken: {
    title: 'This link is incomplete',
    lead: 'Part of it is missing or changed, which often happens when a link wraps across lines. Ask the sender to copy the whole link again.',
  },
  newer: {
    title: 'This link needs a newer Fuselane',
    lead: 'It was made by a newer version. Update Fuselane (free), then open the link again.',
  },
}

const title = document.querySelector<HTMLElement>('[data-state-title]')
const lead = document.querySelector<HTMLElement>('[data-state-lead]')
const okTitle = title?.textContent ?? ''
const okLead = lead?.textContent ?? ''

function render() {
  const state = checkToken(location.hash)
  document.body.dataset.link = state
  document.querySelectorAll<HTMLElement>('[data-ok]').forEach((el) => (el.hidden = state !== 'ok'))
  if (state === 'ok') {
    const token = location.hash.slice(1).trim()
    document.querySelector<HTMLAnchorElement>('[data-open]')!.href = `fuselane://send/${token}`
    if (title) title.textContent = okTitle
    if (lead) lead.textContent = okLead
    return
  }
  if (title) title.textContent = COPY[state].title
  if (lead) {
    lead.textContent = COPY[state].lead
    if (state === 'newer') {
      const a = document.createElement('a')
      a.href = '../download/'
      a.textContent = ' Download the latest version.'
      lead.append(a)
    }
  }
}

document.querySelector('[data-copy-link]')?.addEventListener('click', async (e) => {
  const label = (e.currentTarget as HTMLElement).querySelector('span')
  try {
    await navigator.clipboard.writeText(location.href)
    if (label) {
      label.textContent = 'Copied'
      setTimeout(() => (label.textContent = 'Copy link'), 1800)
    }
  } catch {
    toast('Select the address bar and copy the link from there.')
  }
})

addEventListener('hashchange', render)
render()
