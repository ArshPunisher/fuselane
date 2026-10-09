// Screen changes that move instead of jump (motion 7): the browser's View
// Transitions cross-fade and slide the parts that change. Skipped when the
// system asks for less motion, when the window is hidden, for keyboard use, or where the
// webview doesn't support it (the change then just happens).
import { flushSync } from 'react-dom'

interface Transition {
  ready: Promise<void>
  finished: Promise<void>
  updateCallbackDone: Promise<void>
}
type Doc = Document & { startViewTransition?: (cb: () => void) => Transition }

const ignore = () => {}

// Keyboard changes happen at once: a transition applies its change a frame
// later, and a quick next key (Ctrl+1 then Down) would act on the old screen.
let lastKey = 0
if (typeof addEventListener === 'function')
  addEventListener('keydown', () => (lastKey = performance.now()), { capture: true })

export function withTransition(change: () => void) {
  const doc = document as Doc
  if (
    !doc.startViewTransition ||
    document.hidden ||
    performance.now() - lastKey < 300 ||
    matchMedia('(prefers-reduced-motion: reduce)').matches
  ) {
    change()
    return
  }
  const t = doc.startViewTransition(() => flushSync(change))
  // A quicker second change skips the first one's animation; the change itself
  // still happens, so the rejection is expected and not an error.
  t.ready.catch(ignore)
  t.finished.catch(ignore)
  t.updateCallbackDone.catch(ignore)
}
