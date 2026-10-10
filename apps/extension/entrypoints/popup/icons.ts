// Phosphor icons (the desktop app's set), inlined as SVG text so the popup has no
// requests and no fonts to wait for. They draw in currentColor.
import arrowSquareOut from '@phosphor-icons/core/assets/regular/arrow-square-out.svg?raw'
import check from '@phosphor-icons/core/assets/bold/check-bold.svg?raw'
import desktop from '@phosphor-icons/core/assets/regular/desktop.svg?raw'
import downloadSimple from '@phosphor-icons/core/assets/regular/download-simple.svg?raw'
import file from '@phosphor-icons/core/assets/regular/file.svg?raw'
import filmStrip from '@phosphor-icons/core/assets/regular/film-strip.svg?raw'
import gear from '@phosphor-icons/core/assets/regular/gear.svg?raw'
import musicNote from '@phosphor-icons/core/assets/regular/music-note.svg?raw'
import power from '@phosphor-icons/core/assets/regular/power.svg?raw'
import rss from '@phosphor-icons/core/assets/regular/rss.svg?raw'
import videoCamera from '@phosphor-icons/core/assets/regular/video-camera.svg?raw'

const ICONS = {
  'arrow-square-out': arrowSquareOut,
  check,
  desktop,
  'download-simple': downloadSimple,
  file,
  'film-strip': filmStrip,
  gear,
  'music-note': musicNote,
  power,
  rss,
  'video-camera': videoCamera,
} as const

export type IconName = keyof typeof ICONS

/** Our own bundled SVG text, parsed (not innerHTML, so nothing is ever run). */
function svg(name: IconName): Node {
  const doc = new DOMParser().parseFromString(ICONS[name], 'image/svg+xml')
  return document.importNode(doc.documentElement, true)
}

/** An icon element, hidden from screen readers (the text next to it says it). */
export function icon(name: IconName): HTMLSpanElement {
  const span = document.createElement('span')
  span.className = 'ic'
  span.setAttribute('aria-hidden', 'true')
  span.append(svg(name))
  return span
}

/** Fills every `<span class="ic" data-icon="…">` already in the page. */
export function fillIcons(root: ParentNode = document) {
  for (const el of root.querySelectorAll<HTMLElement>('.ic[data-icon]')) {
    const name = el.dataset.icon as IconName
    if (ICONS[name]) {
      el.replaceChildren(svg(name))
      el.setAttribute('aria-hidden', 'true')
    }
  }
}
