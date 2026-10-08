import { useEffect, useState } from 'react'
import { DownloadSimple, Gear, Plus, ShareNetwork } from '@phosphor-icons/react'
import { useApp, type View } from './lib/store'
import { TransferList } from './components/TransferList'
import { TransferDetail } from './components/TransferDetail'
import { NewDownload } from './components/NewDownload'
import { NetworkList, NetworksView } from './components/NetworksView'
import { SettingsView } from './components/SettingsView'
import { Toast } from './components/Toast'
import { UpdateBanner, UpdatedBanner } from './components/UpdateBanner'
import { SlowToggle } from './components/SlowMode'

type Layout = 'compact' | 'regular' | 'wide'

function useLayout(): Layout {
  const get = (): Layout =>
    innerWidth >= 1024 ? 'wide' : innerWidth >= 640 ? 'regular' : 'compact'
  const [layout, setLayout] = useState(get)
  useEffect(() => {
    const on = () => setLayout(get())
    addEventListener('resize', on)
    return () => removeEventListener('resize', on)
  }, [])
  return layout
}

const NAV: { id: View; label: string; Icon: typeof DownloadSimple }[] = [
  { id: 'transfers', label: 'Downloads', Icon: DownloadSimple },
  { id: 'networks', label: 'Networks', Icon: ShareNetwork },
  { id: 'settings', label: 'Settings', Icon: Gear },
]

function Nav({ kind }: { kind: 'sidebar' | 'rail' | 'tabs' }) {
  const view = useApp((s) => s.view)
  const setView = useApp((s) => s.setView)
  const active = useApp((s) => s.jobs.filter((j) => j.status === 'running').length)
  return (
    <nav className={`nav nav-${kind}`} aria-label="Main">
      {NAV.map(({ id, label, Icon }) => (
        <button
          key={id}
          className="nav-item"
          aria-current={view === id ? 'page' : undefined}
          onClick={() => setView(id)}
          title={kind === 'rail' ? label : undefined}
        >
          <Icon
            size={kind === 'tabs' ? 22 : 18}
            aria-hidden
            weight={view === id ? 'fill' : 'regular'}
          />
          <span className={kind === 'rail' ? 'sr-only' : 'nav-label'}>{label}</span>
          {id === 'transfers' && active > 0 && kind !== 'tabs' && (
            <span className="nav-count num">{active}</span>
          )}
        </button>
      ))}
    </nav>
  )
}

function NewButton({ iconOnly = false }: { iconOnly?: boolean }) {
  const setAdding = useApp((s) => s.setAdding)
  return (
    <button
      className={iconOnly ? 'icon-btn icon-btn-primary' : 'btn btn-primary'}
      onClick={() => setAdding(true)}
      aria-label={iconOnly ? 'New download' : undefined}
    >
      <Plus size={16} aria-hidden weight="bold" />
      {!iconOnly && 'New download'}
    </button>
  )
}

function Brand() {
  const demo = useApp((s) => s.backend?.demo)
  return (
    <div className="brand">
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
      {demo && (
        <span className="chip" title="Running in a browser: these transfers are simulated.">
          Demo data
        </span>
      )}
    </div>
  )
}

function Transfers({ layout }: { layout: Layout }) {
  const jobs = useApp((s) => s.jobs)
  const selected = useApp((s) => s.selected)
  const select = useApp((s) => s.select)
  const job = jobs.find((j) => j.id === selected)

  // On wide windows keep something in the detail pane.
  useEffect(() => {
    if (layout === 'wide' && selected === null && jobs.length) {
      const first = jobs.find((j) => j.status === 'running') ?? jobs[0]
      if (first) select(first.id)
    }
  }, [layout, selected, jobs, select])

  if (layout === 'wide') {
    return (
      <div className="split">
        <div className="pane-list">
          <TransferList />
        </div>
        <div className="pane-detail">
          {job ? (
            <TransferDetail job={job} onBack={null} />
          ) : (
            <div className="detail-empty muted">Pick a download to see it fuse.</div>
          )}
        </div>
      </div>
    )
  }
  if (job) return <TransferDetail job={job} onBack={() => select(null)} />
  return <TransferList />
}

export function App() {
  const start = useApp((s) => s.start)
  const view = useApp((s) => s.view)
  const setAdding = useApp((s) => s.setAdding)
  const layout = useLayout()

  useEffect(() => {
    void start()
  }, [start])

  // Shortcuts: Cmd/Ctrl+N opens the dialog; pasting a link anywhere starts one.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      const k = e.key.toLowerCase()
      if (mod && k === 'n') {
        e.preventDefault()
        setAdding(true)
        return
      }
      if (mod && (k === ',' || k === '1' || k === '2' || k === '3')) {
        e.preventDefault()
        useApp.getState().setView(k === '2' ? 'networks' : k === '1' ? 'transfers' : 'settings')
        return
      }
      // The rest only apply outside text fields and dialogs.
      const t = e.target as HTMLElement | null
      if (mod || e.altKey) return
      if (
        t &&
        (t.tagName === 'INPUT' ||
          t.tagName === 'TEXTAREA' ||
          t.tagName === 'SELECT' ||
          t.isContentEditable)
      )
        return
      if (document.querySelector('dialog[open]')) return
      const s = useApp.getState()
      if (s.view !== 'transfers') return
      const ids = s.jobs.map((j) => j.id)
      const at = s.selected === null ? -1 : ids.indexOf(s.selected)
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        const next = e.key === 'ArrowDown' ? Math.min(ids.length - 1, at + 1) : Math.max(0, at - 1)
        if (ids[next] !== undefined) s.select(ids[next])
      } else if (e.key === 'Escape' && s.selected !== null && innerWidth < 1024) {
        s.select(null)
      } else if (
        e.key === ' ' &&
        s.selected !== null &&
        (!t || t === document.body || t.id === 'main')
      ) {
        const job = s.jobs.find((j) => j.id === s.selected)
        if (!job) return
        e.preventDefault()
        if (job.status === 'running' || job.status === 'queued') void s.act((b) => b.pause(job.id))
        else if (job.resumable) void s.act((b) => b.resume(job.id))
      }
    }
    const paste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return
      const text = e.clipboardData?.getData('text') ?? ''
      if (/^https?:\/\//i.test(text.trim())) setAdding(true, text.trim())
    }
    // Dropping a link (from a browser's address bar or a page) starts one too.
    const over = (e: DragEvent) => {
      if (e.dataTransfer?.types.some((t) => t === 'text/uri-list' || t === 'text/plain'))
        e.preventDefault()
    }
    const drop = (e: DragEvent) => {
      const text = (
        e.dataTransfer?.getData('text/uri-list') ||
        e.dataTransfer?.getData('text/plain') ||
        ''
      )
        .split('\n')
        .map((l) => l.trim())
        .find((l) => l && !l.startsWith('#'))
      if (text && /^https?:\/\//i.test(text)) {
        e.preventDefault()
        setAdding(true, text)
      }
    }
    addEventListener('keydown', key)
    addEventListener('paste', paste)
    addEventListener('dragover', over)
    addEventListener('drop', drop)
    return () => {
      removeEventListener('keydown', key)
      removeEventListener('paste', paste)
      removeEventListener('dragover', over)
      removeEventListener('drop', drop)
    }
  }, [setAdding])

  // Pages appear once the backend is connected, so no button can be clicked into
  // the moment where its action would silently do nothing.
  const connected = useApp((s) => s.backend !== null)
  const page = !connected ? (
    <div className="page" aria-busy="true" aria-label="Starting Fuselane" />
  ) : view === 'networks' ? (
    <NetworksView />
  ) : view === 'settings' ? (
    <SettingsView />
  ) : (
    <Transfers layout={layout} />
  )

  return (
    <div className="app" data-layout={layout}>
      <a className="skip" href="#main">
        Skip to content
      </a>
      {layout === 'wide' && (
        <aside className="sidebar">
          <Brand />
          <NewButton />
          <Nav kind="sidebar" />
          <div className="sidebar-nets">
            <SlowToggle />
            <h2 className="group">Networks</h2>
            <NetworkList compact />
          </div>
        </aside>
      )}
      {layout === 'regular' && (
        <aside className="rail">
          <NewButton iconOnly />
          <Nav kind="rail" />
        </aside>
      )}
      <div className="main-col">
        {layout !== 'wide' && (
          <header className="topbar">
            <Brand />
            {layout === 'compact' && <NewButton iconOnly />}
          </header>
        )}
        <main className="main" id="main" tabIndex={-1}>
          <UpdatedBanner />
          <UpdateBanner />
          {page}
        </main>
        {layout === 'compact' && <Nav kind="tabs" />}
      </div>
      <NewDownload />
      <Toast />
    </div>
  )
}
