/** The Fuselane mark and name: three lanes fusing into one. */
export function Wordmark() {
  return (
    <span className="wordmark" translate="no">
      <svg className="mark" viewBox="0 0 512 512" aria-hidden="true">
        <g fill="none" strokeLinecap="round" strokeWidth="34">
          <path d="M70 150 C 180 150, 220 256, 300 256" stroke="var(--lane-tide)" />
          <path d="M70 256 L 300 256" stroke="var(--lane-volt)" />
          <path d="M70 362 C 180 362, 220 256, 300 256" stroke="var(--lane-iris)" />
          <path d="M300 256 L 442 256" stroke="var(--fuse)" strokeWidth="46" />
        </g>
      </svg>
      Fuselane
    </span>
  )
}
