import {
  CheckCircle,
  DownloadSimple,
  Pause,
  Play,
  WarningCircle,
  HourglassMedium,
  Magnet,
  Plus,
  UploadSimple,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, percent, rateText } from '../lib/format'
import { assignLanes } from '../lib/lanes'
import type { JobView, Live, TorrentView } from '../lib/types'
import { STATUS_WORD } from './status'
import { TORRENT_WORD, torrentNets } from './TorrentDetail'

function StatusIcon({ job }: { job: JobView }) {
  const p = { size: 18, 'aria-hidden': true } as const
  switch (job.status) {
    case 'completed':
      return <CheckCircle {...p} className="ic ic-success" weight="fill" />
    case 'running':
      return <DownloadSimple {...p} className="ic ic-fuse" />
    case 'paused':
      return <Pause {...p} className="ic" />
    case 'queued':
      return <HourglassMedium {...p} className="ic" />
    default:
      return <WarningCircle {...p} className="ic ic-danger" weight="fill" />
  }
}

/** A thin bar split by the networks' shares while running. */
function Bar({ job, live }: { job: JobView; live: Live | undefined }) {
  const written = live?.written ?? job.written
  const total = live?.total ?? job.total
  const pct = job.status === 'completed' ? 100 : (percent(written, total) ?? 0)
  const nets = live?.networks ?? []
  const lanes = assignLanes(nets)
  const sum = nets.reduce((a, n) => a + n.bytes, 0)
  return (
    <div className="bar" data-status={job.status}>
      {job.status === 'running' && sum > 0 ? (
        <div className="bar-fill" style={{ width: `${pct}%` }}>
          {nets.map((n, i) => (
            <span
              key={n.name}
              style={{ width: `${(n.bytes / sum) * 100}%`, background: `var(--lane-${lanes[i]})` }}
            />
          ))}
        </div>
      ) : (
        <div className="bar-fill" style={{ width: `${pct}%` }} />
      )}
    </div>
  )
}

function meta(job: JobView, live: Live | undefined): string {
  const written = live?.written ?? job.written
  const total = live?.total ?? job.total
  if (job.status === 'completed') return total ? bytes(total) : 'Saved'
  const pct = percent(written, total)
  const size = total ? `${bytes(written)} of ${bytes(total)}` : written > 0 ? bytes(written) : ''
  const head = [pct !== null ? `${Math.floor(pct)}%` : '', size].filter(Boolean).join(', ')
  if (job.status === 'running' && live) return `${head}${head ? ', ' : ''}${rateText(live.rate)}`
  return head
}

function Row({ job }: { job: JobView }) {
  const live = useApp((s) => s.live[job.id])
  const selected = useApp((s) => s.selected === job.id)
  const select = useApp((s) => s.select)
  const act = useApp((s) => s.act)
  const showLive = job.status === 'running' ? live : undefined
  const canPause = job.status === 'running' || job.status === 'queued'
  return (
    <li className="row" data-selected={selected || undefined} data-status={job.status}>
      <button
        className="row-main"
        onClick={() => select(job.id)}
        aria-current={selected ? 'true' : undefined}
      >
        <StatusIcon job={job} />
        <span className="row-text">
          <span className="row-name" title={job.name} translate="no">
            {job.name}
          </span>
          <span className="row-meta">
            <span className="row-state">{STATUS_WORD[job.status]}</span>
            <span className="num">{meta(job, showLive)}</span>
          </span>
          <Bar job={job} live={showLive} />
        </span>
      </button>
      {canPause || job.resumable ? (
        <button
          className="icon-btn row-action"
          aria-label={canPause ? `Pause ${job.name}` : `Resume ${job.name}`}
          title={canPause ? 'Pause' : 'Resume'}
          onClick={() => act((b) => (canPause ? b.pause(job.id) : b.resume(job.id)))}
        >
          {canPause ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
        </button>
      ) : (
        <span className="row-action-slot" aria-hidden="true" />
      )}
    </li>
  )
}

function torrentMeta(t: TorrentView): string {
  if (t.status === 'completed') return bytes(t.total)
  if (t.status === 'seeding') return `${bytes(t.total)}, shared ${bytes(t.uploaded)}`
  const pct = percent(t.done, t.total)
  const head = `${pct === null ? 0 : Math.floor(pct)}%, ${bytes(t.done)} of ${bytes(t.total)}`
  return t.status === 'downloading' ? `${head}, ${rateText(t.rate)}` : head
}

function TorrentRow({ t }: { t: TorrentView }) {
  const selected = useApp((s) => s.selectedTorrent === t.id)
  const select = useApp((s) => s.selectTorrent)
  const act = useApp((s) => s.act)
  const known = useApp((s) => s.networks)
  const nets = torrentNets(t.networks, known)
  const lanes = assignLanes(nets)
  const pct =
    t.status === 'completed' || t.status === 'seeding' ? 100 : (percent(t.done, t.total) ?? 0)
  const canPause = t.status === 'downloading' || t.status === 'checking'
  const canResume = t.status === 'paused' || t.status === 'failed'
  const status =
    t.status === 'downloading' ? 'running' : t.status === 'seeding' ? 'completed' : t.status
  const p = { size: 18, 'aria-hidden': true } as const
  return (
    <li className="row" data-selected={selected || undefined} data-status={status}>
      <button
        className="row-main"
        onClick={() => select(t.id)}
        aria-current={selected ? 'true' : undefined}
      >
        {t.status === 'completed' ? (
          <CheckCircle {...p} className="ic ic-success" weight="fill" />
        ) : t.status === 'seeding' ? (
          <UploadSimple {...p} className="ic ic-success" />
        ) : t.status === 'failed' ? (
          <WarningCircle {...p} className="ic ic-danger" weight="fill" />
        ) : (
          <Magnet {...p} className={t.status === 'downloading' ? 'ic ic-fuse' : 'ic'} />
        )}
        <span className="row-text">
          <span className="row-name" title={t.name} translate="no">
            {t.name}
          </span>
          <span className="row-meta">
            <span className="row-state">{TORRENT_WORD[t.status]}</span>
            <span className="num">{torrentMeta(t)}</span>
          </span>
          <div className="bar" data-status={status}>
            <div className="bar-fill" style={{ width: `${pct}%` }}>
              {t.status === 'downloading' &&
                t.done > 0 &&
                nets.map((n, i) => (
                  <span
                    key={n.name}
                    style={{
                      width: `${(n.credited / t.done) * 100}%`,
                      background: `var(--lane-${lanes[i]})`,
                    }}
                  />
                ))}
            </div>
          </div>
        </span>
      </button>
      {canPause || canResume ? (
        <button
          className="icon-btn row-action"
          aria-label={canPause ? `Pause ${t.name}` : `Resume ${t.name}`}
          title={canPause ? 'Pause' : 'Resume'}
          onClick={() => act((b) => (canPause ? b.pauseTorrent(t.id) : b.resumeTorrent(t.id)))}
        >
          {canPause ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
        </button>
      ) : (
        <span className="row-action-slot" aria-hidden="true" />
      )}
    </li>
  )
}

export function TransferList() {
  const jobs = useApp((s) => s.jobs)
  const torrents = useApp((s) => s.torrents)
  const ready = useApp((s) => s.ready)
  const setAdding = useApp((s) => s.setAdding)
  if (!ready) {
    return (
      <ul className="list" aria-busy="true" aria-label="Loading downloads">
        {[0, 1, 2].map((i) => (
          <li key={i} className="row skeleton">
            <span className="sk sk-icon" />
            <span className="row-text">
              <span className="sk sk-line" />
              <span className="sk sk-line short" />
            </span>
          </li>
        ))}
      </ul>
    )
  }
  if (!jobs.length && !torrents.length) {
    return (
      <div className="empty">
        <p className="empty-title">Nothing downloading yet</p>
        <p className="empty-body">
          Paste a link or a magnet, or drop a .torrent file. Fuselane spreads it across every
          network you have, then fuses the parts into one file.
        </p>
        <button className="btn btn-primary" onClick={() => setAdding(true)}>
          <Plus size={16} aria-hidden /> New download
        </button>
      </div>
    )
  }
  const active = jobs.filter((j) => j.status === 'running' || j.status === 'queued')
  const rest = jobs.filter((j) => !(j.status === 'running' || j.status === 'queued'))
  return (
    <div className="list-wrap">
      {active.length > 0 && (
        <section aria-labelledby="g-active">
          <h2 id="g-active" className="group">
            Active <span className="num">{active.length}</span>
          </h2>
          <ul className="list">
            {active.map((j) => (
              <Row key={j.id} job={j} />
            ))}
          </ul>
        </section>
      )}
      {torrents.length > 0 && (
        <section aria-labelledby="g-torrents">
          <h2 id="g-torrents" className="group">
            Torrents <span className="num">{torrents.length}</span>
          </h2>
          <ul className="list">
            {torrents.map((t) => (
              <TorrentRow key={t.id} t={t} />
            ))}
          </ul>
        </section>
      )}
      {rest.length > 0 && (
        <section aria-labelledby="g-rest">
          <h2 id="g-rest" className="group">
            Recent <span className="num">{rest.length}</span>
          </h2>
          <ul className="list">
            {rest.map((j) => (
              <Row key={j.id} job={j} />
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}
