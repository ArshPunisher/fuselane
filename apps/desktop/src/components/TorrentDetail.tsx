import { useEffect, useRef, useState } from 'react'
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  FolderOpen,
  HandPalm,
  Pause,
  Play,
  Trash,
  WarningCircle,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, eta, percent, rate } from '../lib/format'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { usePoll } from '../lib/poll'
import { FilePicker } from './FilePicker'
import { LiveRate } from './LiveRate'
import { Orb } from './Orb'
import { PiecesMap } from './PiecesMap'
import { REVEAL_LABEL } from './TransferDetail'
import type { NetView, PeerView, TorrentFileView, TorrentNetView, TorrentView } from '../lib/types'

export const TORRENT_WORD: Record<TorrentView['status'], string> = {
  checking: 'Checking',
  downloading: 'Downloading',
  paused: 'Paused',
  seeding: 'Sharing',
  completed: 'Done',
  failed: 'Stopped',
}

/** Torrent networks with the names and kinds the Networks page knows. */
export function torrentNets(nets: TorrentNetView[], known: NetView[]) {
  return nets.map((n) => {
    const k = known.find((x) => x.name === n.name)
    return { ...n, label: k?.label ?? n.name, kind: k?.kind ?? 'other' }
  })
}

function RemoveTorrent({ t }: { t: TorrentView }) {
  const act = useApp((s) => s.act)
  const [confirm, setConfirm] = useState(false)
  useEffect(() => {
    if (!confirm) return
    const id = setTimeout(() => setConfirm(false), 6000)
    return () => clearTimeout(id)
  }, [confirm])
  if (!confirm)
    return (
      <button className="btn btn-ghost" onClick={() => setConfirm(true)}>
        <Trash size={16} aria-hidden /> Remove
      </button>
    )
  return (
    <span className="confirm" role="group" aria-label="Remove this torrent">
      <button className="btn" onClick={() => act((b) => b.removeTorrent(t.id, false))}>
        Keep files
      </button>
      <button className="btn btn-danger" onClick={() => act((b) => b.removeTorrent(t.id, true))}>
        Delete files
      </button>
    </span>
  )
}

/** Which files to download, changeable while the torrent runs. */
/** Who the torrent is talking to: app, address, which network, speed both ways. */
function Peers({
  t,
  peers,
  known,
}: {
  t: TorrentView
  peers: PeerView[] | null
  known: NetView[]
}) {
  const nets = torrentNets(t.networks, known)
  const lanes = assignLanes(nets)
  if (!peers) return <div className="sk sk-line" aria-busy="true" aria-label="Loading peers" />
  if (!peers.length)
    return (
      <p className="muted peers-empty">
        {t.status === 'downloading' || t.status === 'checking'
          ? 'Looking for peers…'
          : 'No peers connected right now.'}
      </p>
    )
  return (
    <ul className="peers" aria-label="Connected peers">
      {peers.map((p) => {
        const i = nets.findIndex((n) => n.name === p.network)
        const net = i >= 0 ? nets[i] : undefined
        return (
          <li key={p.addr} className="peer-row">
            <span className="peer-who">
              <span className="peer-client">{p.client ?? 'Unknown app'}</span>
              <span className="peer-addr num" translate="no">
                {p.addr}
              </span>
            </span>
            <span className="peer-net">
              {net ? (
                <>
                  <span className="peer-dot" style={{ background: `var(--lane-${lanes[i]})` }} />
                  {netTitle(net)}
                </>
              ) : (
                'Came to you'
              )}
            </span>
            <span className="peer-rate num" title="From this peer">
              <ArrowDown size={12} aria-label="Down" />
              {p.down > 0 ? rate(p.down).value + ' ' + rate(p.down).unit : '0'}
            </span>
            <span className="peer-rate num up" title="To this peer">
              <ArrowUp size={12} aria-label="Up" />
              {p.up > 0 ? rate(p.up).value + ' ' + rate(p.up).unit : '0'}
            </span>
          </li>
        )
      })}
    </ul>
  )
}

type Tab = 'networks' | 'peers' | 'files'

function Files({ t }: { t: TorrentView }) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [files, setFiles] = useState<TorrentFileView[] | null>(null)
  const [chosen, setChosen] = useState<Set<number>>(new Set())
  const [failed, setFailed] = useState(false)
  const shownFor = useRef<string | null>(null)
  useEffect(() => {
    let live = true
    // A new torrent starts from a skeleton; a refresh keeps the old list visible.
    if (shownFor.current !== t.id) setFiles(null)
    shownFor.current = t.id
    backend
      ?.torrentFiles(t.id)
      .then((f) => {
        if (!live) return
        setFiles(f)
        setChosen(new Set(f.filter((x) => x.selected).map((x) => x.index)))
      })
      .catch(() => live && setFailed(true))
    return () => {
      live = false
    }
  }, [backend, t.id, t.selectedCount])
  if (failed) return <p className="field-help">The file list couldn't be loaded.</p>
  if (!files) return <div className="sk sk-line" aria-busy="true" aria-label="Loading files" />
  if (!files.length) return null
  const saved = new Set(files.filter((f) => f.selected).map((f) => f.index))
  const changed = saved.size !== chosen.size || [...chosen].some((i) => !saved.has(i))
  return (
    <section aria-labelledby="tf-title" className="torrent-files">
      <h2 id="tf-title" className="group">
        Files
      </h2>
      <FilePicker files={files} chosen={chosen} onChange={setChosen} idPrefix={`tf-${t.id}`} />
      {changed && (
        <div className="pick-actions">
          <button className="btn btn-ghost" onClick={() => setChosen(saved)}>
            Undo
          </button>
          <button
            className="btn btn-primary"
            disabled={chosen.size === 0}
            onClick={() => act((b) => b.selectTorrentFiles(t.id, [...chosen]))}
          >
            Save choice
          </button>
        </div>
      )}
    </section>
  )
}

export function TorrentDetail({ t, onBack }: { t: TorrentView; onBack: (() => void) | null }) {
  const act = useApp((s) => s.act)
  const known = useApp((s) => s.networks)
  const nets = torrentNets(t.networks, known)
  const lanes = assignLanes(nets)
  const pct =
    t.status === 'completed' || t.status === 'seeding' ? 100 : (percent(t.done, t.total) ?? 0)
  const canPause = t.status === 'downloading' || t.status === 'checking'
  const canResume = t.status === 'paused' || t.status === 'failed'
  const left = eta(t.total - t.done, t.rate)
  const maxPeers = Math.max(1, ...nets.map((n) => n.peers))
  const backend = useApp((s) => s.backend)
  const [tab, setTab] = useState<Tab>('networks')
  const filesOk = t.status !== 'completed' && t.status !== 'seeding'
  const shownTab = tab === 'files' && !filesOk ? 'networks' : tab
  const cells = usePoll(() => backend?.torrentPieces(t.id, 256) ?? Promise.resolve([]), 1000, [
    backend,
    t.id,
  ])
  const peers = usePoll(
    () => (shownTab === 'peers' && backend ? backend.torrentPeers(t.id) : Promise.resolve(null)),
    1000,
    [backend, t.id, shownTab],
  )
  const peerCount = nets.reduce((a, n) => a + n.peers, 0)
  const tabs: [Tab, string][] = [
    ['networks', 'Networks'],
    ['peers', 'Peers'],
    ...(filesOk ? ([['files', 'Files']] as [Tab, string][]) : []),
  ]

  return (
    <article className="detail" aria-labelledby="td-title">
      <header className="detail-head">
        {onBack && (
          <button className="icon-btn" onClick={onBack} aria-label="Back to downloads">
            <ArrowLeft size={18} aria-hidden />
          </button>
        )}
        <div className="detail-title-wrap">
          <h1 id="td-title" className="detail-title" title={t.name} translate="no">
            {t.name}
          </h1>
          <p className="detail-sub" title={t.folder} translate="no">
            {t.folder}
          </p>
        </div>
        <div className="detail-actions">
          {canPause && (
            <button className="btn" onClick={() => act((b) => b.pauseTorrent(t.id))}>
              <Pause size={16} aria-hidden /> Pause
            </button>
          )}
          {t.status === 'seeding' && (
            <button className="btn" onClick={() => act((b) => b.stopSharing(t.id))}>
              <HandPalm size={16} aria-hidden /> Stop sharing
            </button>
          )}
          {canResume && (
            <button className="btn btn-primary" onClick={() => act((b) => b.resumeTorrent(t.id))}>
              <Play size={16} aria-hidden /> Resume
            </button>
          )}
          <button className="btn" onClick={() => act((b) => b.revealTorrent(t.id))}>
            <FolderOpen size={16} aria-hidden /> {REVEAL_LABEL}
          </button>
          <RemoveTorrent t={t} />
        </div>
      </header>

      {t.error && (
        <div className="notice" role="alert">
          <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
          <div className="notice-body">
            <p>{t.error}</p>
          </div>
        </div>
      )}

      <div className="torrent-body">
        <div className="torrent-head">
          {t.status === 'downloading' ? (
            <LiveRate value={t.rate} />
          ) : (
            <p className="speed num">
              {Math.floor(pct)}
              <span className="unit">%</span>
            </p>
          )}
          <div
            className="bar torrent-bar"
            data-status={t.status === 'completed' ? 'completed' : 'running'}
          >
            <div className="bar-fill" style={{ width: `${pct}%` }}>
              {t.done > 0 &&
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
          <dl className="facts">
            <div>
              <dt>Saved</dt>
              <dd className="num">
                {bytes(t.done)}
                {t.status !== 'completed' && <span className="of"> of {bytes(t.total)}</span>}
              </dd>
            </div>
            <div>
              <dt>{t.status === 'downloading' ? 'Time left' : 'Status'}</dt>
              <dd className="num">
                {t.status === 'downloading' ? left || 'Working it out' : TORRENT_WORD[t.status]}
              </dd>
            </div>
            {t.uploaded > 0 && (
              <div>
                <dt>Shared</dt>
                <dd className="num">
                  {bytes(t.uploaded)}
                  {t.total > 0 && (
                    <span className="of"> ({(t.uploaded / t.total).toFixed(2)}x)</span>
                  )}
                </dd>
              </div>
            )}
            <div>
              <dt>Files</dt>
              <dd className="num">
                {t.selectedCount} of {t.fileCount}
              </dd>
            </div>
          </dl>
        </div>

        {cells && cells.length > 0 && (
          <PiecesMap cells={cells} label={`Pieces: ${Math.floor(pct)}% here`} />
        )}

        <div className="tabs" role="tablist" aria-label="Torrent details">
          {tabs.map(([id, label]) => (
            <button
              key={id}
              role="tab"
              type="button"
              id={`tt-${id}`}
              aria-selected={shownTab === id}
              aria-controls={`tp-${id}`}
              onClick={() => setTab(id)}
            >
              {label}
              {id === 'peers' && peerCount > 0 && (
                <span className="tab-count num">{peerCount}</span>
              )}
            </button>
          ))}
        </div>
        <div
          className="tab-panel"
          role="tabpanel"
          id={`tp-${shownTab}`}
          aria-labelledby={`tt-${shownTab}`}
        >
          {shownTab === 'networks' && (
            <>
              {nets.length > 0 && (
                <table className="nets">
                  <caption className="sr-only">Networks in this torrent</caption>
                  <thead>
                    <tr>
                      <th scope="col">Network</th>
                      <th scope="col" className="r">
                        Peers
                      </th>
                      <th scope="col" className="r">
                        Share
                      </th>
                      <th scope="col" className="r">
                        Verified
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {nets.map((n, i) => (
                      <tr key={n.name}>
                        <td>
                          <span className="net-cell">
                            <Orb
                              lane={lanes[i] ?? 'steel'}
                              speed={n.peers / maxPeers}
                              state={t.status === 'downloading' && n.peers > 0 ? 'live' : 'idle'}
                            />
                            <span>
                              <span className="net-name">{netTitle(n)}</span>
                              <span className="net-kind">
                                {kindLabel(n.kind)}, {n.name}
                              </span>
                            </span>
                          </span>
                        </td>
                        <td className="r num">{n.peers}</td>
                        <td className="r num">
                          {t.done > 0 ? `${Math.round((n.credited / t.done) * 100)}%` : ''}
                        </td>
                        <td className="r num">{bytes(n.credited)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              <p className="field-help">
                Each network is credited only with pieces that passed their checksum.
              </p>
            </>
          )}
          {shownTab === 'peers' && <Peers t={t} peers={peers} known={known} />}
          {shownTab === 'files' && <Files t={t} />}
        </div>
      </div>
    </article>
  )
}
