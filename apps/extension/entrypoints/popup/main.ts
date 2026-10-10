import { normalizeRules } from '@fuselane/capture'
import { appStatus } from '../../lib/status.ts'
import { offerLink } from '../../lib/handoff.ts'
import { findOnPage, mediaList, type MediaItem } from '../../lib/media.ts'
import { getSession } from '../../lib/session.ts'
import './style.css'

const status = document.querySelector<HTMLParagraphElement>('#status')!
const enabled = document.querySelector<HTMLInputElement>('#enabled')!

async function load() {
  const { rules } = await browser.storage.local.get('rules')
  enabled.checked = normalizeRules(rules).enabled
  const s = await appStatus((m) =>
    browser.runtime.sendNativeMessage('app.fuselane.host', m as object),
  )
  status.textContent = s.text
}

enabled.addEventListener('change', async () => {
  const { rules } = await browser.storage.local.get('rules')
  await browser.storage.local.set({ rules: { ...normalizeRules(rules), enabled: enabled.checked } })
})

document.querySelector('#settings')!.addEventListener('click', (e) => {
  e.preventDefault()
  void browser.runtime.openOptionsPage()
  window.close()
})

const KIND = { video: 'Video', audio: 'Audio', link: 'File' } as const

/** Lists the tab's videos, audio and file links (activeTab: only now, only this tab). */
async function onThisPage() {
  const section = document.querySelector<HTMLElement>('#page')!
  const list = document.querySelector<HTMLUListElement>('#media')!
  const said = document.querySelector<HTMLParagraphElement>('#media-status')!
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true })
  if (!tab?.id || !tab.url || !/^https?:/.test(tab.url)) return
  let items: MediaItem[] = []
  try {
    const [res] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: findOnPage,
    })
    items = mediaList((res?.result as Parameters<typeof mediaList>[0]) ?? [], tab.url)
  } catch {
    return // a page the browser doesn't let extensions read
  }
  section.hidden = false
  // A video page (YouTube and many more): the app finds the real video and its
  // qualities, then downloads it over every network.
  const video = document.querySelector<HTMLButtonElement>('#video')!
  video.addEventListener('click', async () => {
    video.disabled = true
    const reply = (await browser.runtime
      .sendNativeMessage('app.fuselane.host', { v: 1, type: 'page.video', url: tab.url })
      .catch(() => null)) as { type?: string } | null
    if (reply?.type === 'page.video.opened') {
      window.close()
    } else {
      video.disabled = false
      said.textContent =
        "Fuselane isn't running, or this version can't get videos yet. Open it and try again."
    }
  })
  if (!items.length) {
    said.textContent = 'No videos or file links here.'
    return
  }
  for (const m of items) {
    const li = document.createElement('li')
    const text = document.createElement('span')
    text.className = 'm-name'
    text.title = m.url
    text.textContent = m.name
    const kind = document.createElement('span')
    kind.className = 'm-kind'
    kind.textContent = m.label ? `${KIND[m.kind]} · ${m.label}` : KIND[m.kind]
    text.append(kind)
    const button = document.createElement('button')
    button.type = 'button'
    button.textContent = 'Download'
    button.setAttribute('aria-label', `Download ${m.name}`)
    button.addEventListener('click', async () => {
      button.disabled = true
      const took = await offerLink(
        m.url,
        tab.url,
        (msg) => browser.runtime.sendNativeMessage('app.fuselane.host', msg as object),
        getSession,
        (u) => browser.downloads.download({ url: u }),
      )
      button.textContent = took ? 'Sent' : 'In browser'
      said.textContent = took
        ? `${m.name} is downloading in Fuselane.`
        : `Fuselane isn't running, so the browser is downloading ${m.name}.`
    })
    li.append(text, button)
    list.append(li)
  }
}

void load()
void onThisPage()
