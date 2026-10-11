// Chrome and Edge only: the extension's own downloads run here, because the
// background service worker can stop at any time. Events go back to it.
import { createRunner } from '../../lib/runner.ts'
import type { SelfJob } from '../../lib/selfjobs.ts'

const runner = createRunner((e) => void browser.runtime.sendMessage(e).catch(() => {}))

browser.runtime.onMessage.addListener((m: unknown, sender) => {
  if (sender.id !== browser.runtime.id) return
  const msg = (m ?? {}) as {
    target?: string
    cmd?: string
    job?: SelfJob
    limit?: number
    id?: string
  }
  if (msg.target !== 'offscreen') return
  if (msg.cmd === 'start' && msg.job && typeof msg.limit === 'number')
    void runner.start(msg.job, msg.limit)
  else if (msg.cmd === 'pause' && msg.id) runner.pause(msg.id)
  else if (msg.cmd === 'cancel' && msg.id) void runner.cancel(msg.id)
  else if (msg.cmd === 'saved' && msg.id) void runner.saved(msg.id)
})
