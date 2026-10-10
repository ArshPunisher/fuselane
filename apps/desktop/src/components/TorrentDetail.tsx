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
import { FuseCore } from './FuseCore'
import { Stream } from './Stream'
import { BIN, RemoveDialog } from './RemoveDialog'
import { Orb } from './Orb'
import { PiecesMap } from './PiecesMap'
import { REVEAL_LABEL } from './TransferDetail'
import { mark, t, tb, tn } from '../lib/i18n'
import type { History } from '../lib/store'
import type {
  FilePriority,
  JobView,
  Live,
  NetView,
  PeerView,
  TorrentNetView,
  TorrentView,
} from '../lib/types'

export const TORRENT_WORD: Record<TorrentView['status'], string> = {
  checking: mark('Checking'),
  downloading: mark('Downloading'),
  paused: mark('Paused'),
  seeding: mark('Sharing'),
  completed: mark('Done'),
  failed: mark('Stopped'),
}

/** A torrent's state in a word, in the language in use. */
export function torrentWord(s: TorrentView['status']): string {
  return t(TORRENT_WORD[s])
}

/** Ticks round the ring, like a download's. */
const TICKS = 180

/** Golden-ratio steps: spreads each network's share evenly round the ring. */
const GOLDEN = 0.6180339887

/**
 * A torrent as the ring a download shows: its pieces fill the ticks, and each
 * filled tick takes the colour of a network in proportion to the verified data
 * that network carried (pieces don't say which one fetched them, the shares
 * do). Pieces part-way in are in flight on the networks moving data now.
 */
function torrentCore(
  tor: TorrentView,
  nets: (TorrentNetView & { label: string; kind: string })[],
  cells: number[] | null,
): { job: JobView; live: Live } {
  const status: JobView['status'] =
    tor.status === 'downloading' || tor.status === 'checking'
      ? 'running'
      : tor.status === 'completed' || tor.status === 'seeding'
        ? 'completed'
        : tor.status === 'failed'
          ? 'failed'
          : 'paused'
  const credited = nets.reduce((a, n) => a + n.credited, 0)
  const cumulative: number[] = []
  let run = 0
  for (const n of nets) {
    run += credited ? n.credited / credited : 1 / Math.max(1, nets.length)
    cumulative.push(run)
  }
  const moving = nets.map((n, i) => (n.rate > 0 ? i : -1)).filter((i) => i >= 0)
  const frac = tor.total ? tor.done / tor.total : 0
  const ticks: number[] = []
  for (let i = 0; i < TICKS; i++) {
    const fill = cells?.length
      ? (cells[Math.min(cells.length - 1, Math.floor((i * cells.length) / TICKS))] ?? 0)
      : i < frac * TICKS
        ? 100
        : 0
    const pick = (i * GOLDEN) % 1
    const owner = fill >= 100 ? cumulative.findIndex((c) => pick < c) + 1 : 0
    const inflight =
      fill > 0 && fill < 100 && moving.length ? (moving[i % moving.length] ?? 0) + 1 : 0
    ticks.push(fill, owner, inflight)
  }
  const job = {
    id: -1,
    status,
    written: tor.done,
    total: tor.total,
  } as JobView
  const live: Live = {
    id: -1,
    written: tor.done,
    total: tor.total,
    rate: tor.rate,
    networks: nets.map((n) => ({
      name: n.name,
      label: n.label,
      kind: n.kind,
      bytes: n.credited,
      rate: n.rate,
      streams: n.peers,
      dead: status === 'running' && n.peers === 0,
    })),
    ticks,
    retries: 0,
    hedges: 0,
  }
  return { job, live }
}

/** Five samples a second of each network's speed, for the stream graph. */
function useRateHistory(names: string[], rates: number[]): History {
  const latest = useRef({ names, rates })
  latest.current = { names, rates }
  const [h, setH] = useState<History>({ names, rates: [] })
  useEffect(() => {
    const timer = setInterval(() => {
      setH((prev) => {
        const { names: n, rates: r } = latest.current
        const same = prev.names.join() === n.join()
        return { names: n, rates: [...(same ? prev.rates : []), r].slice(-300) }
      })
    }, 200)
    return () => clearInterval(timer)
  }, [])
  return h
}

/** The middle of the ring: the speed while it downloads, otherwise how far. */
function TorrentCenter({ tor, live, pct }: { tor: TorrentView; live: Live; pct: number }) {
  if (tor.status === 'downloading') {
    const moving = live.networks.filter((n) => n.rate > 0)
    const best = moving.reduce<(typeof moving)[number] | null>(
      (a, n) => (!a || n.rate > a.rate ? n : a),
      null,
    )
    const total = moving.reduce((a, n) => a + n.rate, 0)
    const faster = moving.length > 1 && best && best.rate > 0 ? total / best.rate : 0
    return (
      <>
        <LiveRate value={total} />
        <p className="speed-sub">
          {faster >= 1.1 && best
            ? t('{times}x faster than {network}', {
                times: faster.toFixed(1),
                network: netTitle(best),
              })
            : tn(Math.max(1, moving.length), '{n} network', '{n} networks')}
        </p>
      </>
    )
  }
  return (
    <>
      <p className="speed num">
        {Math.floor(pct)}
        <span className="unit">%</span>
      </p>
      <p className="speed-sub">{torrentWord(tor.status)}</p>
    </>
  )
}

/** Torrent networks with the names and kinds the Networks page knows. */
export function torrentNets(nets: TorrentNetView[], known: NetView[]) {
  return nets.map((n) => {
    const k = known.find((x) => x.name === n.name)
    return { ...n, label: k?.label ?? n.name, kind: k?.kind ?? 'other' }
  })
}

function RemoveTorrent({ t: tor }: { t: TorrentView }) {
  const act = useApp((s) => s.act)
  const [open, setOpen] = useState(false)
  const finished = tor.status === 'completed' || tor.status === 'seeding'
  return (
    <>
      <button className="btn btn-ghost" onClick={() => setOpen(true)}>
        <Trash size={16} aria-hidden /> {t('Remove')}
      </button>
      <RemoveDialog
        open={open}
        onClose={() => setOpen(false)}
        title={t('Remove this torrent?')}
        text={
          finished
            ? t("{name} leaves your list. What should happen to what's already saved?", {
                name: tor.name,
              })
            : t("{name} stops and leaves your list. What should happen to what's already saved?", {
                name: tor.name,
              })
        }
        where={tor.folder}
        facts={
          finished
            ? t('{size}, {selected} of {count} files', {
                size: bytes(tor.total),
                selected: tor.selectedCount,
                count: tor.fileCount,
              })
            : t('{done} of {total} saved', { done: bytes(tor.done), total: bytes(tor.total) })
        }
        choices={[
          { label: t('Keep files'), run: () => act((b) => b.removeTorrent(tor.id, false)) },
          {
            label: t('Move files to {bin}', { bin: t(BIN) }),
            danger: true,
            run: () => act((b) => b.removeTorrent(tor.id, true)),
          },
        ]}
      />
    </>
  )
}

/** Which files to download, changeable while the torrent runs. */
/** Who the torrent is talking to: app, address, which network, speed both ways. */
function Peers({
  t: tor,
  peers,
  known,
}: {
  t: TorrentView
  peers: PeerView[] | null
  known: NetView[]
}) {
  const nets = torrentNets(tor.networks, known)
  const lanes = assignLanes(nets)
  if (!peers) return <div className="sk sk-line" aria-busy="true" aria-label={t('Loading peers')} />
  if (!peers.length)
    return (
      <p className="muted peers-empty">
        {tor.status === 'downloading' || tor.status === 'checking'
          ? t('Looking for peers…')
          : t('No peers connected right now.')}
      </p>
    )
  return (
    <ul className="peers" aria-label={t('Connected peers')}>
      {peers.map((p) => {
        const i = nets.findIndex((n) => n.name === p.network)
        const net = i >= 0 ? nets[i] : undefined
        return (
          <li key={p.addr} className="peer-row">
            <span className="peer-who">
              <span className="peer-client">{p.client ?? t('Unknown app')}</span>
              <span className="peer-addr num" translate="no">
                {p.addr}
              </span>
            </span>
            <span className="peer-net">
              {net ? (
                <>
                  <span className="peer-dot" style={{ background: `var(--lane-${lanes[i]})` }} />
                  <span translate="no">{netTitle(net)}</span>
                </>
              ) : (
                t('Came to you')
              )}
            </span>
            <span className="peer-rate num" title={t('From this peer')}>
              <ArrowDown size={12} aria-label={t('Down')} />
              {p.down > 0 ? rate(p.down).value + ' ' + rate(p.down).unit : '0'}
            </span>
            <span className="peer-rate num up" title={t('To this peer')}>
              <ArrowUp size={12} aria-label={t('Up')} />
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
  { id: 'high', label: mark('High'), hint: mark('Fetched before the others') },
  { id: 'normal', label: mark('Normal'), hint: '' },
  { id: 'low', label: mark('Low'), hint: mark('Waits until the rest is done') },
  { id: 'skip', label: mark('Skip'), hint: mark('Not downloaded') },
]

const MEDIA = /\.(mp4|m4v|mkv|webm|mov|avi|ts|mp3|m4a|flac|ogg|opus|wav)$/i

/** A torrent's files: how much of each is here, its priority, and Play (B8.10). */
function Files({ t: tor }: { t: TorrentView }) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const files = usePoll(() => backend?.torrentFiles(tor.id) ?? Promise.resolve(null), 1500, [
    backend,
    tor.id,
    tor.selectedCount,
  ])
  const [playing, setPlaying] = useState<{ file: number; url: string } | null>(null)
  const [copied, setCopied] = useState(false)
  if (!files) return <div className="sk sk-line" aria-busy="true" aria-label={t('Loading files')} />
  if (!files.length) return null
  const wanted = files.filter((f) => f.priority !== 'skip')
  const wantedBytes = wanted.reduce((a, f) => a + f.size, 0)
  const live = tor.status === 'downloading' || tor.status === 'paused' || tor.status === 'checking'
  return (
    <section aria-labelledby="tf-title" className="torrent-files">
      <div className="files-head">
        <h2 id="tf-title" className="group">
          {t('Files')}{' '}
          <span className="num">
            {t('{done} of {total}', { done: wanted.length, total: files.length })}
          </span>
        </h2>
        <span className="num muted">{t('{size} to download', { size: bytes(wantedBytes) })}</span>
      </div>
      <table className="files">
        <caption className="sr-only">{t('Files in this torrent')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('Name')}</th>
            <th scope="col" className="r">
              {t('Size')}
            </th>
            <th scope="col" className="r">
              {t('Here')}
            </th>
            <th scope="col" className="r">
              {t('Priority')}
            </th>
            <th scope="col">
              <span className="sr-only">{t('Play')}</span>
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
                    aria-label={t('Priority of {name}', { name })}
                    value={f.priority}
                    onChange={(e) =>
                      void act((b) =>
                        b.setFilePriority(tor.id, f.index, e.target.value as FilePriority),
                      )
                    }
                  >
                    {PRIORITIES.map((p) => (
                      <option key={p.id} value={p.id} title={p.hint ? t(p.hint) : undefined}>
                        {t(p.label)}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="r">
                  {live && MEDIA.test(f.path) && f.priority !== 'skip' && (
                    <button
                      className="btn btn-sm"
                      aria-label={t('Play {name}', { name })}
                      onClick={async () => {
                        if (!backend) return
                        try {
                          const url = await backend.playTorrentFile(tor.id, f.index)
                          setCopied(false)
                          setPlaying({ file: f.index, url })
                        } catch (err) {
                          await act(() => Promise.reject(err))
                        }
                      }}
                    >
                      <Play size={14} weight="fill" aria-hidden /> {t('Play')}
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
            {t(
              "Playing from the start while the rest arrives. If it doesn't open in your player, paste this link into one (VLC, IINA, mpv):",
            )}
          </p>
          <div className="field-row">
            <input readOnly className="num" value={playing.url} aria-label={t('Stream link')} />
            <button
              className="btn"
              onClick={() => {
                void navigator.clipboard?.writeText(playing.url).then(() => setCopied(true))
              }}
            >
              <Copy size={16} aria-hidden /> {copied ? t('Copied') : t('Copy')}
            </button>
          </div>
        </div>
      ) : (
        <p className="field-help">
          {t(
            'High files come first and Low ones wait for the rest. Play fetches the start of a video or song first, so it can begin while the rest arrives.',
          )}
        </p>
      )}
    </section>
  )
}

export function TorrentDetail({ t: tor, onBack }: { t: TorrentView; onBack: (() => void) | null }) {
  const act = useApp((s) => s.act)
  const known = useApp((s) => s.networks)
  const nets = torrentNets(tor.networks, known)
  const lanes = assignLanes(nets)
  const pct =
    tor.status === 'completed' || tor.status === 'seeding'
      ? 100
      : (percent(tor.done, tor.total) ?? 0)
  const canPause = tor.status === 'downloading' || tor.status === 'checking'
  const canResume = tor.status === 'paused' || tor.status === 'failed'
  const left = eta(tor.total - tor.done, tor.rate)
  const maxPeers = Math.max(1, ...nets.map((n) => n.peers))
  const backend = useApp((s) => s.backend)
  const [tab, setTab] = useState<Tab>('networks')
  const filesOk = tor.status !== 'completed' && tor.status !== 'seeding'
  const shownTab = tab === 'files' && !filesOk ? 'networks' : tab
  const cells = usePoll(() => backend?.torrentPieces(tor.id, 256) ?? Promise.resolve([]), 1000, [
    backend,
    tor.id,
  ])
  const peers = usePoll(
    () => (shownTab === 'peers' && backend ? backend.torrentPeers(tor.id) : Promise.resolve(null)),
    1000,
    [backend, tor.id, shownTab],
  )
  const peerCount = nets.reduce((a, n) => a + n.peers, 0)
  const core = torrentCore(tor, nets, cells)
  const history = useRateHistory(
    nets.map((n) => n.name),
    nets.map((n) => n.rate),
  )
  const tabs: [Tab, string][] = [
    ['networks', mark('Networks')],
    ['peers', mark('Peers')],
    ...(filesOk ? ([['files', mark('Files')]] as [Tab, string][]) : []),
  ]

  return (
    <article className="detail" aria-labelledby="td-title">
      <header className="detail-head">
        {onBack && (
          <button className="icon-btn" onClick={onBack} aria-label={t('Back to downloads')}>
            <ArrowLeft size={18} aria-hidden />
          </button>
        )}
        <div className="detail-title-wrap">
          <h1 id="td-title" className="detail-title" title={tor.name} translate="no">
            {tor.name}
          </h1>
          <p className="detail-sub" title={tor.folder} translate="no">
            {tor.folder}
          </p>
        </div>
        <div className="detail-actions">
          {canPause && (
            <button className="btn" onClick={() => act((b) => b.pauseTorrent(tor.id))}>
              <Pause size={16} aria-hidden /> {t('Pause')}
            </button>
          )}
          {tor.status === 'seeding' && (
            <button className="btn" onClick={() => act((b) => b.stopSharing(tor.id))}>
              <HandPalm size={16} aria-hidden /> {t('Stop sharing')}
            </button>
          )}
          {canResume && (
            <button className="btn btn-primary" onClick={() => act((b) => b.resumeTorrent(tor.id))}>
              <Play size={16} aria-hidden /> {t('Resume')}
            </button>
          )}
          <button className="btn" onClick={() => act((b) => b.revealTorrent(tor.id))}>
            <FolderOpen size={16} aria-hidden /> {t(REVEAL_LABEL)}
          </button>
          <RemoveTorrent t={tor} />
        </div>
      </header>

      {tor.error && (
        <div className="notice" role="alert">
          <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
          <div className="notice-body">
            <p>{tb(tor.error)}</p>
          </div>
        </div>
      )}

      <div className="torrent-body">
        <div className="detail-body">
          <FuseCore
            job={core.job}
            live={core.live}
            center={<TorrentCenter tor={tor} live={core.live} pct={pct} />}
          />
          <div className="detail-side">
            <dl className="facts">
              <div>
                <dt>{t('Saved')}</dt>
                <dd className="num">
                  {bytes(tor.done)}
                  {tor.status !== 'completed' && (
                    <span className="of"> {t('of {total}', { total: bytes(tor.total) })}</span>
                  )}
                </dd>
              </div>
              <div>
                <dt>{tor.status === 'downloading' ? t('Time left') : t('Status')}</dt>
                <dd className="num">
                  {tor.status === 'downloading'
                    ? left || t('Working it out')
                    : torrentWord(tor.status)}
                </dd>
              </div>
              <div>
                <dt>{t('Peers')}</dt>
                <dd className="num">{peerCount}</dd>
              </div>
              {tor.uploaded > 0 && (
                <div>
                  <dt>{t('Shared')}</dt>
                  <dd className="num">
                    {bytes(tor.uploaded)}
                    {tor.total > 0 && (
                      <span className="of"> ({(tor.uploaded / tor.total).toFixed(2)}x)</span>
                    )}
                  </dd>
                </div>
              )}
              <div>
                <dt>{t('Files')}</dt>
                <dd className="num">
                  {t('{done} of {total}', { done: tor.selectedCount, total: tor.fileCount })}
                </dd>
              </div>
            </dl>
            {tor.status === 'downloading' && <Stream history={history} lanes={lanes} />}
          </div>
        </div>

        {cells && cells.length > 0 && (
          <PiecesMap cells={cells} label={t('Pieces: {pct}% here', { pct: Math.floor(pct) })} />
        )}

        <div className="tabs" role="tablist" aria-label={t('Torrent details')}>
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
              {t(label)}
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
                  <caption className="sr-only">{t('Networks in this torrent')}</caption>
                  <thead>
                    <tr>
                      <th scope="col">{t('Network')}</th>
                      {tor.status === 'downloading' && (
                        <th scope="col" className="r">
                          {t('Speed')}
                        </th>
                      )}
                      <th scope="col" className="r">
                        {t('Peers')}
                      </th>
                      <th scope="col" className="r">
                        {t('Share')}
                      </th>
                      <th scope="col" className="r">
                        {t('Verified')}
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
                              state={tor.status === 'downloading' && n.peers > 0 ? 'live' : 'idle'}
                            />
                            <span>
                              <span className="net-name" translate="no">
                                {netTitle(n)}
                              </span>
                              <span className="net-kind">
                                {kindLabel(n.kind)}, <span translate="no">{n.name}</span>
                              </span>
                            </span>
                          </span>
                        </td>
                        {tor.status === 'downloading' && (
                          <td className="r num">{rateText(n.rate)}</td>
                        )}
                        <td className="r num">{n.peers}</td>
                        <td className="r num">
                          {tor.done > 0 ? `${Math.round((n.credited / tor.done) * 100)}%` : ''}
                        </td>
                        <td className="r num">{bytes(n.credited)}</td>
                      </tr>
                    ))}
                  </tbody>
                  {nets.length > 1 && (
                    <tfoot>
                      <tr>
                        <td>{t('All networks')}</td>
                        {tor.status === 'downloading' && (
                          <td className="r num">{rateText(tor.rate)}</td>
                        )}
                        <td className="r num">{peerCount}</td>
                        <td className="r num">{tor.done > 0 ? '100%' : ''}</td>
                        <td className="r num">{bytes(tor.done)}</td>
                      </tr>
                    </tfoot>
                  )}
                </table>
              )}
              <p className="field-help">
                {tor.status === 'downloading'
                  ? t(
                      'Speed is what each network receives right now. Share counts only pieces that passed their checksum.',
                    )
                  : t('Each network is credited only with pieces that passed their checksum.')}
              </p>
            </>
          )}
          {shownTab === 'peers' && <Peers t={tor} peers={peers} known={known} />}
          {shownTab === 'files' && <Files t={tor} />}
        </div>
      </div>
    </article>
  )
}
