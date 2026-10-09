import { useEffect, useRef, useState } from 'react'
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  Copy,
  FolderOpen,
  HandPalm,
  Pause,
  Play,
  Trash,
  WarningCircle,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, eta, percent, rate, rateText } from '../lib/format'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { usePoll } from '../lib/poll'
import { LiveRate } from './LiveRate'
import { SpeedSplit } from './SpeedSplit'
import { BIN, RemoveDialog } from './RemoveDialog'
import { Orb } from './Orb'
import { PiecesMap } from './PiecesMap'
import { REVEAL_LABEL } from './TransferDetail'
import type { FilePriority, NetView, PeerView, TorrentNetView, TorrentView } from '../lib/types'

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
  const [open, setOpen] = useState(false)
  const finished = t.status === 'completed' || t.status === 'seeding'
  return (
    <>
      <button className="btn btn-ghost" onClick={() => setOpen(true)}>
        <Trash size={16} aria-hidden /> Remove
      </button>
      <RemoveDialog
        open={open}
        onClose={() => setOpen(false)}
        title="Remove this torrent?"
        text={`${t.name} ${finished ? 'leaves your list' : 'stops and leaves your list'}. What should happen to what's already saved?`}
        where={t.folder}
        facts={
          finished
            ? `${bytes(t.total)}, ${t.selectedCount} of ${t.fileCount} files`
            : `${bytes(t.done)} of ${bytes(t.total)} saved`
        }
        choices={[
          { label: 'Keep files', run: () => act((b) => b.removeTorrent(t.id, false)) },
          {
            label: `Move files to ${BIN}`,
            danger: true,
            run: () => act((b) => b.removeTorrent(t.id, true)),
          },
        ]}
      />
    </>
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

const PRIORITIES: { id: FilePriority; label: string; hint: string }[] = [
  { id: 'high', label: 'High', hint: 'Fetched before the others' },
  { id: 'normal', label: 'Normal', hint: '' },
  { id: 'low', label: 'Low', hint: 'Waits until the rest is done' },
  { id: 'skip', label: 'Skip', hint: 'Not downloaded' },
]

const MEDIA = /\.(mp4|m4v|mkv|webm|mov|avi|ts|mp3|m4a|flac|ogg|opus|wav)$/i

/** A torrent's files: how much of each is here, its priority, and Play (B8.10). */
function Files({ t }: { t: TorrentView }) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const files = usePoll(() => backend?.torrentFiles(t.id) ?? Promise.resolve(null), 1500, [
    backend,
    t.id,
    t.selectedCount,
  ])
  const [playing, setPlaying] = useState<{ file: number; url: string } | null>(null)
  const [copied, setCopied] = useState(false)
  if (!files) return <div className="sk sk-line" aria-busy="true" aria-label="Loading files" />
  if (!files.length) return null
  const wanted = files.filter((f) => f.priority !== 'skip')
  const wantedBytes = wanted.reduce((a, f) => a + f.size, 0)
  const live = t.status === 'downloading' || t.status === 'paused' || t.status === 'checking'
  return (
    <section aria-labelledby="tf-title" className="torrent-files">
      <div className="files-head">
        <h2 id="tf-title" className="group">
          Files{' '}
          <span className="num">
            {wanted.length} of {files.length}
          </span>
        </h2>
        <span className="num muted">{bytes(wantedBytes)} to download</span>
      </div>
      <table className="files">
        <caption className="sr-only">Files in this torrent</caption>
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col" className="r">
              Size
            </th>
            <th scope="col" className="r">
              Here
            </th>
            <th scope="col" className="r">
              Priority
            </th>
            <th scope="col">
              <span className="sr-only">Play</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {files.map((f) => {
            const pct = f.size ? Math.floor((f.done / f.size) * 100) : 100
            const name = f.path.split('/').pop() ?? f.path
            return (
              <tr key={f.index} data-skip={f.priority === 'skip' || undefined}>
                <td>
                  <span className="file-name" translate="no" title={f.path}>
                    {f.path}
                  </span>
                </td>
                <td className="r num">{bytes(f.size)}</td>
                <td className="r">
                  <span className="here num">
                    {f.priority === 'skip' ? '' : `${pct}%`}
                    <span className="mini-bar" aria-hidden>
                      <span style={{ width: `${f.priority === 'skip' ? 0 : pct}%` }} />
                    </span>
                  </span>
                </td>
                <td className="r">
                  <select
                    className="prio"
                    data-level={f.priority}
                    aria-label={`Priority of ${name}`}
                    value={f.priority}
                    onChange={(e) =>
                      void act((b) =>
                        b.setFilePriority(t.id, f.index, e.target.value as FilePriority),
                      )
                    }
                  >
                    {PRIORITIES.map((p) => (
                      <option key={p.id} value={p.id} title={p.hint || undefined}>
                        {p.label}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="r">
                  {live && MEDIA.test(f.path) && f.priority !== 'skip' && (
                    <button
                      className="btn btn-sm"
                      aria-label={`Play ${name}`}
                      onClick={async () => {
                        if (!backend) return
                        try {
                          const url = await backend.playTorrentFile(t.id, f.index)
                          setCopied(false)
                          setPlaying({ file: f.index, url })
                        } catch (err) {
                          await act(() => Promise.reject(err))
                        }
                      }}
                    >
                      <Play size={14} weight="fill" aria-hidden /> Play
                    </button>
                  )}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
      {playing ? (
        <div className="play-note" role="status">
          <p>
            Playing from the start while the rest arrives. If it doesn&apos;t open in your player,
            paste this link into one (VLC, IINA, mpv):
          </p>
          <div className="field-row">
            <input readOnly className="num" value={playing.url} aria-label="Stream link" />
            <button
              className="btn"
              onClick={() => {
                void navigator.clipboard?.writeText(playing.url).then(() => setCopied(true))
              }}
            >
              <Copy size={16} aria-hidden /> {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
        </div>
      ) : (
        <p className="field-help">
          High files come first and Low ones wait for the rest. Play fetches the start of a video or
          song first, so it can begin while the rest arrives.
        </p>
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
            <>
              <LiveRate value={t.rate} />
              <SpeedSplit
                parts={nets.map((n, i) => ({
                  name: netTitle(n),
                  lane: lanes[i] ?? 'steel',
                  rate: n.rate,
                }))}
              />
            </>
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
                      {t.status === 'downloading' && (
                        <th scope="col" className="r">
                          Speed
                        </th>
                      )}
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
                        {t.status === 'downloading' && (
                          <td className="r num">{rateText(n.rate)}</td>
                        )}
                        <td className="r num">{n.peers}</td>
                        <td className="r num">
                          {t.done > 0 ? `${Math.round((n.credited / t.done) * 100)}%` : ''}
                        </td>
                        <td className="r num">{bytes(n.credited)}</td>
                      </tr>
                    ))}
                  </tbody>
                  {nets.length > 1 && (
                    <tfoot>
                      <tr>
                        <td>All networks</td>
                        {t.status === 'downloading' && (
                          <td className="r num">{rateText(t.rate)}</td>
                        )}
                        <td className="r num">{peerCount}</td>
                        <td className="r num">{t.done > 0 ? '100%' : ''}</td>
                        <td className="r num">{bytes(t.done)}</td>
                      </tr>
                    </tfoot>
                  )}
                </table>
              )}
              <p className="field-help">
                {t.status === 'downloading'
                  ? 'Speed is what each network receives right now. Share counts only pieces that passed their checksum.'
                  : 'Each network is credited only with pieces that passed their checksum.'}
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
