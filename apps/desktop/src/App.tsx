import { useEffect, useState } from 'react'
import { DownloadSimple, Gear, Plus, ShareNetwork } from '@phosphor-icons/react'
import { useApp, type View } from './lib/store'
import { TransferList } from './components/TransferList'
import { TransferDetail } from './components/TransferDetail'
import { NewDownload } from './components/NewDownload'
import { NetworkList, NetworksView } from './components/NetworksView'
import { SettingsView } from './components/SettingsView'
import { Toast } from './components/Toast'

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
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'n') {
        e.preventDefault()
        setAdding(true)
      }
    }
    const paste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return
      const text = e.clipboardData?.getData('text') ?? ''
      if (/^https?:\/\//i.test(text.trim())) setAdding(true)
    }
    addEventListener('keydown', key)
    addEventListener('paste', paste)
    return () => {
      removeEventListener('keydown', key)
      removeEventListener('paste', paste)
    }
  }, [setAdding])

  const page =
    view === 'networks' ? (
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
          {page}
        </main>
        {layout === 'compact' && <Nav kind="tabs" />}
      </div>
      <NewDownload />
      <Toast />
    </div>
  )
}
