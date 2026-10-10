import { ICONS } from './icon-data'

/**
 * An inline Phosphor icon, rendered into the HTML as a bare <svg> (no
 * wrapper, no JavaScript), exactly where the plain site put one.
 */
export function Icon({ name }: { name: string }) {
  const svg = ICONS[name]
  if (!svg) return null
  const viewBox = /viewBox="([^"]+)"/.exec(svg)?.[1] ?? '0 0 256 256'
  const inner = svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '')
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox={viewBox}
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
      dangerouslySetInnerHTML={{ __html: inner }}
    />
  )
}
