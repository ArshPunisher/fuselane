/**
 * A rolling-digit counter: each digit is a 0-9 strip that slides to its value.
 * Separators stay put. Columns are reused while the number keeps its shape.
 * The element is decorative (aria-hidden); the exact figure lives elsewhere.
 */
export function odometer(el: HTMLElement) {
  let shape = ''
  return (text: string) => {
    const next = text.replace(/\d/g, '0')
    if (next !== shape) {
      shape = next
      el.replaceChildren(
        ...[...text].map((ch) => {
          const cell = document.createElement('span')
          if (!/\d/.test(ch)) {
            cell.className = 'odo-sep'
            cell.textContent = ch
            return cell
          }
          cell.className = 'odo-d'
          const strip = document.createElement('span')
          strip.className = 'odo-strip'
          strip.textContent = '0123456789'
          cell.append(strip)
          return cell
        }),
      )
    }
    const strips = el.querySelectorAll<HTMLElement>('.odo-strip')
    let i = 0
    for (const ch of text) {
      if (!/\d/.test(ch)) continue
      strips[i++]?.style.setProperty('--n', ch)
    }
  }
}
