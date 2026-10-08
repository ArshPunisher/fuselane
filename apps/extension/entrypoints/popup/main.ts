import { normalizeRules } from '@fuselane/capture'
import { appStatus } from '../../lib/status.ts'
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

void load()
