import { DEFAULT_RULES, type Rules } from '@fuselane/capture'
import './style.css'

const status = document.querySelector<HTMLParagraphElement>('#status')!
const enabled = document.querySelector<HTMLInputElement>('#enabled')!

async function load() {
  const { rules } = await browser.storage.local.get('rules')
  const r: Rules = { ...DEFAULT_RULES, ...(rules as Partial<Rules> | undefined) }
  enabled.checked = r.enabled
  try {
    const pong = (await browser.runtime.sendNativeMessage('app.fuselane.host', {
      type: 'ping',
    })) as {
      app?: { version?: string }
      error?: string
    }
    status.textContent = pong.app?.version
      ? `Connected to Fuselane ${pong.app.version}.`
      : 'Fuselane is installed but not running. Open it to hand downloads over.'
  } catch {
    status.textContent =
      'Fuselane isn’t installed on this computer, so downloads stay in the browser.'
  }
}

enabled.addEventListener('change', async () => {
  const { rules } = await browser.storage.local.get('rules')
  await browser.storage.local.set({
    rules: { ...(rules as object | undefined), enabled: enabled.checked },
  })
})

void load()
