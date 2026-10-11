// "Too big for the extension" (no app installed): get the app, or let the
// browser download this one file (REVIEW-2026-10-11 D).
import { formatSize } from '@fuselane/capture'
import { fillIcons } from '../popup/icons.ts'
import '../popup/style.css'

const q = new URLSearchParams(location.search)
const url = q.get('url') ?? ''
const name = q.get('name') ?? 'This file'
const size = Number(q.get('size'))

document.querySelector('#big-text')!.textContent =
  `${name}${size > 0 ? ` is ${formatSize(size)}, ` : ' is '}more than the extension downloads by itself. ` +
  'The free app takes any size and splits it across Wi‑Fi, Ethernet and your phone at once.'

const done = (choice: 'app' | 'browser') => {
  void browser.runtime.sendMessage({ type: 'decide', choice, url }).finally(() => window.close())
}
document.querySelector<HTMLAnchorElement>('#get')!.addEventListener('click', (e) => {
  e.preventDefault()
  done('app')
})
document
  .querySelector<HTMLButtonElement>('#browser')!
  .addEventListener('click', () => done('browser'))
fillIcons()
