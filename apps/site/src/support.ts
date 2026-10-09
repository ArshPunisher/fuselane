// Support page: Share uses the system's share sheet, or copies the link.
import { toast } from './common'

const url = 'https://arshpunisher.github.io/fuselane/'

document.querySelector<HTMLButtonElement>('[data-share]')?.addEventListener('click', async () => {
  const data = {
    title: 'Fuselane',
    text: 'A free download manager that uses Wi-Fi, Ethernet and your phone at once.',
    url,
  }
  try {
    if (navigator.share) {
      await navigator.share(data)
      return
    }
  } catch (e) {
    if ((e as DOMException).name === 'AbortError') return
  }
  try {
    await navigator.clipboard.writeText(url)
    toast('Link copied')
  } catch {
    toast(url)
  }
})
