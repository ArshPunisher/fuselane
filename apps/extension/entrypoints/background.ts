import { normalizeRules, type Rules } from '@fuselane/capture'
import { handOff, offerFor, type Item } from '../lib/handoff.ts'

const HOST = 'app.fuselane.host'

async function rules(): Promise<Rules> {
  const { rules } = await browser.storage.local.get('rules')
  // Cleaned: stored rules may come from an older version or a hand edit.
  return normalizeRules(rules)
}

const ask = (message: unknown) => browser.runtime.sendNativeMessage(HOST, message as object)

const downloads = {
  pause: (id: number) => browser.downloads.pause(id),
  resume: (id: number) => browser.downloads.resume(id),
  cancel: (id: number) => browser.downloads.cancel(id),
  erase: async (id: number) => {
    await browser.downloads.erase({ id })
  },
}

function item(d: Browser.downloads.DownloadItem): Item {
  return {
    id: d.id,
    state: d.state,
    url: d.url,
    finalUrl: d.finalUrl,
    referrer: d.referrer,
    filename: d.filename,
    mime: d.mime,
    size: d.totalBytes > 0 ? d.totalBytes : d.fileSize,
  }
}

export default defineBackground(() => {
  browser.downloads.onCreated.addListener(async (d) => {
    await handOff(item(d), await rules(), downloads, ask)
  })

  browser.runtime.onInstalled.addListener(() => {
    browser.contextMenus.create({
      id: 'fuselane-download',
      title: 'Download with Fuselane',
      contexts: ['link', 'video', 'audio'],
    })
  })

  browser.contextMenus.onClicked.addListener(async (info) => {
    const url = info.linkUrl ?? info.srcUrl
    if (info.menuItemId !== 'fuselane-download' || !url) return
    const offer = offerFor(
      { id: -1, state: 'in_progress', url, referrer: info.pageUrl },
      'contextMenu',
    )
    let accepted = false
    try {
      const reply = (await ask(offer)) as { type?: string } | undefined
      accepted = reply?.type === 'download.accepted'
    } catch {
      accepted = false
    }
    // The app couldn't take it: the browser downloads it instead.
    if (!accepted) await browser.downloads.download({ url })
  })
})
