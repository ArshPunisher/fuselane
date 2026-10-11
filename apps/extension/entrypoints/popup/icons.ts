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
import globe from '@phosphor-icons/core/assets/regular/globe.svg?raw'
import pause from '@phosphor-icons/core/assets/fill/pause-fill.svg?raw'
import play from '@phosphor-icons/core/assets/fill/play-fill.svg?raw'
import x from '@phosphor-icons/core/assets/regular/x.svg?raw'
import trash from '@phosphor-icons/core/assets/regular/trash.svg?raw'
import folderOpen from '@phosphor-icons/core/assets/regular/folder-open.svg?raw'
import gauge from '@phosphor-icons/core/assets/regular/gauge.svg?raw'
import listIcon from '@phosphor-icons/core/assets/regular/list.svg?raw'
import lockOpen from '@phosphor-icons/core/assets/regular/lock-open.svg?raw'
import arrowClockwise from '@phosphor-icons/core/assets/regular/arrow-clockwise.svg?raw'
import warningCircle from '@phosphor-icons/core/assets/regular/warning-circle.svg?raw'
import checkCircle from '@phosphor-icons/core/assets/fill/check-circle-fill.svg?raw'
import disc from '@phosphor-icons/core/assets/regular/disc.svg?raw'
import fileZip from '@phosphor-icons/core/assets/regular/file-zip.svg?raw'

const ICONS = {
  disc,
  'file-zip': fileZip,
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
  globe,
  pause,
  play,
  x,
  trash,
  'folder-open': folderOpen,
  gauge,
  list: listIcon,
  'lock-open': lockOpen,
  'arrow-clockwise': arrowClockwise,
  'warning-circle': warningCircle,
  'check-circle': checkCircle,
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
