import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import {
  Check,
  DesktopTower,
  DeviceMobile,
  DownloadSimple,
  Eye,
  FolderOpen,
  Laptop,
  LockKey,
  PaperPlaneTilt,
  QrCode,
  ShieldCheck,
  TextT,
  Copy,
  X,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes } from '../lib/format'
import { intlLocale, mark, t, tn, tr } from '../lib/i18n'
import type {
  DeviceView,
  NearbyRequest,
  NearbyTransfer,
  PhoneView,
  SyncView,
  TrustedDevice,
} from '../lib/types'

/** "Mac", "Windows", "Linux", or what a LocalSend device says it is. */
function deviceSub(d: Pick<DeviceView, 'model' | 'fuselane' | 'kind'>): string {
  const m = d.model ?? ''
  if (d.fuselane) {
    if (/macos/i.test(m)) return 'Mac'
    if (/windows/i.test(m)) return 'Windows'
    if (/linux/i.test(m)) return 'Linux'
    return 'Fuselane'
  }
  const what = m || (d.kind === 'mobile' ? t('Phone') : t('Computer'))
  return t('{what}, via LocalSend', { what })
}

function DeviceIcon({
  kind,
  model,
  size = 22,
}: {
  kind: string
  model: string | null
  size?: number
}) {
  if (kind === 'mobile') return <DeviceMobile size={size} aria-hidden />
  if (/windows/i.test(model ?? '')) return <DesktopTower size={size} aria-hidden />
  return <Laptop size={size} aria-hidden />
}

/** "2 Oct" from "2026-10-02", the way the system writes dates. */
function shortDate(iso: string): string {
  const d = new Date(`${iso}T12:00:00`)
  return Number.isNaN(d.getTime())
    ? iso
    : d.toLocaleDateString(intlLocale(), { day: 'numeric', month: 'short' })
}

/** Seconds left, counting down every second from the backend's last value. */
function useSecondsLeft(seconds: number | null): number | null {
  const until = useRef(0)
  const [, tick] = useState(0)
  useEffect(() => {
    until.current = seconds === null ? 0 : Date.now() + seconds * 1000
  }, [seconds])
  useEffect(() => {
    if (seconds === null) return
    const timer = setInterval(() => tick((n) => n + 1), 1000)
    return () => clearInterval(timer)
  }, [seconds])
  if (seconds === null) return null
  return Math.max(0, Math.round((until.current - Date.now()) / 1000))
}

const live = (x: NearbyTransfer) =>
  x.state === 'asking' || x.state === 'sending' || x.state === 'receiving'

/** A ring that fills with progress (0..1); just the track when unknown. */
function Ring({
  value,
  size,
  stroke = 3,
}: {
  value: number | null
  size: number
  stroke?: number
}) {
  const r = (size - stroke) / 2
  const c = 2 * Math.PI * r
  return (
    <svg className="ring" width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden>
      <circle className="ring-track" cx={size / 2} cy={size / 2} r={r} strokeWidth={stroke} />
      {value !== null && (
        <circle
          className="ring-fill"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={stroke}
          strokeDasharray={c}
          strokeDashoffset={c * (1 - Math.max(0, Math.min(1, value)))}
        />
      )}
    </svg>
  )
}

/** Sends the picked files to a device. */
function useSendTo() {
  const act = useApp((s) => s.act)
  return (fingerprint: string) =>
    act(async (b) => {
      const paths = await b.nearbyPick()
      if (paths.length) await b.nearbySend(fingerprint, paths)
    })
}

/** Where each device sits around this computer: evenly spaced, starting at the top. */
function orbit(n: number, i: number): { x: number; y: number } {
  const angle = (-90 + (360 / Math.max(1, n)) * i + (n === 2 ? 30 : 0)) * (Math.PI / 180)
  return { x: 50 + 34 * Math.cos(angle), y: 46 + 33 * Math.sin(angle) }
}

/**
 * The space view: this computer in the middle, devices around it. A slow sweep
 * looks for devices (motion 1); found ones spring in; files dragged over one lift
 * it (motion 2); a transfer runs as a beam of dots between the two (motion 3) with
 * a progress ring around the device (motion 4).
 */
function Radar({
  me,
  devices,
  transfers,
  over,
}: {
  me: string
  devices: DeviceView[]
  transfers: NearbyTransfer[]
  over: string | null
}) {
  const sendTo = useSendTo()
  const active = (d: DeviceView) => transfers.find((x) => x.device === d.alias && live(x))
  return (
    <div className="radar" data-searching={devices.length === 0 || undefined}>
      <div className="radar-rings" aria-hidden>
        <i />
        <i />
        <i />
        <span className="radar-sweep" />
      </div>
      <svg className="radar-beams" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden>
        {devices.map((d, i) => {
          const x = active(d)
          if (!x) return null
          const p = orbit(devices.length, i)
          const inward = x.direction === 'in'
          return (
            <line
              key={d.fingerprint}
              className="beam"
              data-wait={x.state === 'asking' || undefined}
              x1={inward ? p.x : 50}
              y1={inward ? p.y : 50}
              x2={inward ? 50 : p.x}
              y2={inward ? 50 : p.y}
              vectorEffect="non-scaling-stroke"
            />
          )
        })}
      </svg>
      <div className="radar-me">
        <span className="radar-me-tile">
          <Laptop size={26} aria-hidden />
        </span>
        <span className="radar-me-name" translate="no">
          {me}
        </span>
        <span className="radar-me-sub">{t('This computer')}</span>
      </div>
      {devices.length === 0 && (
        <p className="radar-empty">
          {t('No one here yet. Open Fuselane or LocalSend on the other device, on this Wi-Fi.')}
        </p>
      )}
      {devices.map((d, i) => {
        const p = orbit(devices.length, i)
        const x = active(d)
        const progress = x && x.state !== 'asking' && x.size ? x.done / x.size : null
        return (
          <button
            key={d.fingerprint}
            type="button"
            className="radar-node"
            data-device={d.fingerprint}
            data-over={over === d.fingerprint || undefined}
            data-busy={x ? x.state : undefined}
            style={{ '--x': `${p.x}%`, '--y': `${p.y}%`, '--i': i } as CSSProperties}
            onClick={() => void sendTo(d.fingerprint)}
            aria-label={t('Send files to {name}', { name: d.alias })}
            title={t('Click to choose files, or drop files here')}
          >
            <span className="radar-node-tile">
              <Ring value={x ? progress : null} size={64} />
              <DeviceIcon kind={d.kind} model={d.model} />
              {d.trusted && (
                <span className="radar-trust" title={t('Trusted')}>
                  <ShieldCheck size={12} weight="fill" aria-hidden />
                </span>
              )}
            </span>
            <span className="radar-node-name" translate="no">
              {d.alias}
            </span>
            <span className="radar-node-sub">
              {x
                ? x.state === 'asking'
                  ? t('Waiting for them…')
                  : `${Math.floor((progress ?? 0) * 100)}%`
                : deviceSub(d)}
            </span>
          </button>
        )
      })}
    </div>
  )
}

/** "Everyone, 10 min" as a ring draining around the eye (motion 6). */
function Visibility({ seconds }: { seconds: number | null }) {
  const act = useApp((s) => s.act)
  const left = useSecondsLeft(seconds)
  const everyone = seconds !== null
  return (
    <section className="visibility" aria-labelledby="vis-label">
      <span className="vis-ic" aria-hidden>
        <Ring value={left === null ? null : left / 600} size={40} stroke={2.5} />
        <Eye size={18} />
      </span>
      <div>
        <p className="setting-name" id="vis-label">
          {t('Who can see this computer')}
        </p>
        <p className="muted">
          {everyone && left !== null
            ? tr('Everyone here, for {time} more.', {
                time: (
                  <span className="num">
                    {Math.floor(left / 60)}:{String(left % 60).padStart(2, '0')}
                  </span>
                ),
              })
            : t('Only devices you trust.')}
        </p>
      </div>
      <div className="segmented" role="radiogroup" aria-labelledby="vis-label">
        <button
          type="button"
          role="radio"
          aria-checked={!everyone}
          onClick={() => act((b) => b.nearbySetEveryone(false))}
        >
          {t('Trusted only')}
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={everyone}
          onClick={() => act((b) => b.nearbySetEveryone(true))}
        >
          {t('Everyone, 10 min')}
        </button>
      </div>
    </section>
  )
}

const STATE_WORD: Record<NearbyTransfer['state'], string> = {
  asking: mark('Waiting for them to accept'),
  sending: mark('Sending'),
  receiving: mark('Receiving'),
  done: mark('Done'),
  declined: mark('They declined'),
  failed: mark("Didn't arrive"),
  cancelled: mark('Cancelled'),
}

/** One transfer: slides in, fills its ring, turns into a check when done (motion 5). */
function Transfer({ x }: { x: NearbyTransfer }) {
  const act = useApp((s) => s.act)
  const running = live(x)
  const pct = x.size ? Math.min(1, x.done / x.size) : 0
  return (
    <li className="activity" data-state={x.state} data-dir={x.direction}>
      <span className="activity-ic" aria-hidden>
        <Ring
          value={x.state === 'done' ? 1 : running && x.state !== 'asking' ? pct : null}
          size={38}
        />
        {x.text !== null && x.state === 'done' ? (
          <TextT size={16} weight="bold" className="done-check" />
        ) : x.state === 'done' ? (
          <Check size={16} weight="bold" className="done-check" />
        ) : x.direction === 'out' ? (
          <PaperPlaneTilt size={16} />
        ) : (
          <DownloadSimple size={16} />
        )}
      </span>
      <span className="activity-text">
        <span className="activity-name" translate="no" title={x.text ?? x.name}>
          {x.text !== null ? `“${x.name}”` : x.name}
        </span>
        <span className="activity-meta">
          {tr(x.direction === 'out' ? 'To {device}.' : 'From {device}.', {
            device: <span translate="no">{x.device}</span>,
          })}{' '}
          {x.error ?? t(STATE_WORD[x.state])}
          {running && x.size > 0 && x.state !== 'asking' && (
            <span>
              {' '}
              {tr('{done} of {total}', {
                done: <span className="num">{bytes(x.done)}</span>,
                total: <span className="num">{bytes(x.size)}</span>,
              })}
            </span>
          )}
        </span>
        {running && x.state !== 'asking' && (
          <span className="activity-bar" aria-hidden>
            <span style={{ width: `${pct * 100}%` }} />
          </span>
        )}
      </span>
      <span className="activity-actions">
        {running ? (
          <button
            className="btn btn-ghost btn-sm"
            aria-label={t('Cancel {name}', { name: x.name })}
            onClick={() => act((b) => b.nearbyCancel(x.id))}
          >
            {t('Cancel')}
          </button>
        ) : (
          <>
            {x.text !== null && x.state === 'done' && (
              <button
                className="btn btn-sm"
                aria-label={t('Copy text from {name}', { name: x.device })}
                onClick={() => void navigator.clipboard?.writeText(x.text ?? '')}
              >
                <Copy size={16} aria-hidden /> {t('Copy')}
              </button>
            )}
            {x.direction === 'in' && x.state === 'done' && x.path && (
              <button className="btn btn-sm" onClick={() => act((b) => b.nearbyReveal(x.id))}>
                <FolderOpen size={16} aria-hidden /> {t('Show')}
              </button>
            )}
            <button
              className="icon-btn"
              aria-label={t('Clear {name}', { name: x.name })}
              onClick={() => act((b) => b.nearbyClear(x.id))}
            >
              <X size={16} aria-hidden />
            </button>
          </>
        )}
      </span>
    </li>
  )
}

/**
 * Text between your devices (B10.2): send what you copied, or type something.
 * A trusted Fuselane computer puts it on its clipboard; LocalSend shows it.
 */
function TextCard({ devices }: { devices: DeviceView[] }) {
  const act = useApp((s) => s.act)
  const [to, setTo] = useState('')
  const [text, setText] = useState('')
  const target = devices.find((d) => d.fingerprint === to) ?? devices[0]
  if (!devices.length) return null
  return (
    <section className="text-card" aria-labelledby="text-title">
      <h2 className="group" id="text-title">
        {t('Send text')}
      </h2>
      <textarea
        aria-label={t('Text to send')}
        rows={2}
        maxLength={65536}
        placeholder={t('Type or paste, or leave empty to send what you copied')}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <div className="text-card-row">
        <select
          aria-label={t('Send to')}
          value={target?.fingerprint ?? ''}
          onChange={(e) => setTo(e.target.value)}
        >
          {devices.map((d) => (
            <option key={d.fingerprint} value={d.fingerprint} translate="no">
              {d.alias}
            </option>
          ))}
        </select>
        <button
          className="btn"
          disabled={!target}
          onClick={() =>
            target &&
            void act(async (b) => {
              await b.nearbySendText(target.fingerprint, text.trim() ? text : null)
              setText('')
            })
          }
        >
          <PaperPlaneTilt size={16} aria-hidden />{' '}
          {text.trim() ? t('Send') : t('Send what I copied')}
        </button>
      </div>
    </section>
  )
}

const SYNC_WORD = {
  'up-to-date': mark('Up to date'),
  sending: mark('Sending changes'),
  waiting: mark('Waiting'),
  problem: mark('Problem'),
} as const

/** "Up to date, 128 files", "Sending changes, 3 to go", "Waiting". */
function syncWord(s: SyncView): string {
  if (s.state === 'sending' && s.pending) return t('Sending changes, {n} to go', { n: s.pending })
  if (s.state === 'up-to-date') return tn(s.files, 'Up to date, {n} file', 'Up to date, {n} files')
  return t(SYNC_WORD[s.state])
}

/**
 * Folders kept in sync with your own trusted computers (B10.3): new and
 * changed files go across whenever both are on the network. One way, and
 * nothing is deleted on the other side.
 */
function SyncCard({ syncs, trusted }: { syncs: SyncView[]; trusted: TrustedDevice[] }) {
  const act = useApp((s) => s.act)
  const backend = useApp((s) => s.backend)
  const [to, setTo] = useState('')
  if (!trusted.length && !syncs.length) return null
  const target = trusted.find((d) => d.fingerprint === to) ?? trusted[0]
  return (
    <section className="sync-card" aria-labelledby="sync-title">
      <h2 className="group" id="sync-title">
        {t('Folders kept in sync')}
      </h2>
      {syncs.length > 0 && (
        <ul className="sync-list">
          {syncs.map((s) => (
            <li key={s.id} data-state={s.state}>
              <span className="sync-text">
                <span className="sync-name" translate="no" title={s.folder}>
                  {s.name} → {s.device}
                </span>
                <span className="muted">
                  {syncWord(s)}
                  {s.note ? `. ${s.note}` : ''}
                </span>
              </span>
              <button
                className="icon-btn"
                aria-label={t('Stop syncing {name}', { name: s.name })}
                onClick={() => void act((b) => b.nearbySyncRemove(s.id))}
              >
                <X size={16} aria-hidden />
              </button>
            </li>
          ))}
        </ul>
      )}
      {trusted.length > 0 && (
        <div className="text-card-row">
          <select
            aria-label={t('Keep in sync with')}
            value={target?.fingerprint ?? ''}
            onChange={(e) => setTo(e.target.value)}
          >
            {trusted.map((d) => (
              <option key={d.fingerprint} value={d.fingerprint} translate="no">
                {d.alias}
              </option>
            ))}
          </select>
          <button
            className="btn"
            onClick={async () => {
              if (!backend || !target) return
              const folder = await backend.pickFolder()
              if (folder) void act((b) => b.nearbySyncAdd(folder, target.fingerprint))
            }}
          >
            <FolderOpen size={16} aria-hidden /> {t('Add a folder')}
          </button>
        </div>
      )}
      <p className="muted sync-help">
        {t(
          "New and changed files go to that computer's downloads folder whenever both are on this network. Nothing is deleted there.",
        )}
      </p>
    </section>
  )
}

/** A phone without an app: show a code; it opens a page from this computer (B8.12). */
function PhoneCard({ phone }: { phone: PhoneView | null }) {
  const act = useApp((s) => s.act)
  const [text, setText] = useState('')
  if (!phone)
    return (
      <section className="phone-card">
        <span className="phone-card-ic" aria-hidden>
          <QrCode size={20} />
        </span>
        <div>
          <p className="setting-name">{t('A phone without Fuselane?')}</p>
          <p className="muted">{t('It can send and receive in its browser, on this Wi-Fi.')}</p>
        </div>
        <button className="btn btn-sm" onClick={() => act((b) => b.nearbyPhone(true))}>
          <QrCode size={16} aria-hidden /> {t('Show a code to scan')}
        </button>
      </section>
    )
  return (
    <section className="phone-card phone-on">
      <div className="phone-code">
        {/* The SVG is made by the app from the link; nothing from outside goes in here. */}
        <span
          className="qr"
          role="img"
          aria-label={t("Code to scan with the phone's camera")}
          dangerouslySetInnerHTML={{ __html: phone.qr }}
        />
        <div className="phone-text">
          <p className="setting-name">{t("Scan with the phone's camera")}</p>
          <p className="muted">
            {tr('Or type {url}', {
              url: (
                <span className="num" translate="no">
                  {phone.url}
                </span>
              ),
            })}
          </p>
        </div>
      </div>
      {phone.offers.length > 0 && (
        <ul className="offers" aria-label={t('Offered to the phone')}>
          {phone.offers.map((o) => (
            <li key={o.id}>
              <span translate="no">{o.name}</span>
              <span className="num muted">{bytes(o.size)}</span>
              <button
                className="icon-btn"
                aria-label={t('Stop offering {name}', { name: o.name })}
                onClick={() => act((b) => b.nearbyPhoneOffer([], o.id))}
              >
                <X size={14} aria-hidden />
              </button>
            </li>
          ))}
        </ul>
      )}
      {phone.text !== null ? (
        <div className="phone-note">
          <p className="muted">
            {tr("On the phone's page: {text}", { text: <q translate="no">{phone.text}</q> })}
          </p>
          <button
            className="icon-btn"
            aria-label={t("Take the text away from the phone's page")}
            onClick={() => act((b) => b.nearbyPhoneText(null, true))}
          >
            <X size={14} aria-hidden />
          </button>
        </div>
      ) : (
        <div className="phone-say">
          <input
            type="text"
            name="phone-text"
            autoComplete="off"
            aria-label={t('Text for the phone')}
            maxLength={65536}
            placeholder={t('Text or a link for the phone, or leave empty for what you copied')}
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
          <button
            className="btn btn-sm"
            onClick={() =>
              act(async (b) => {
                await b.nearbyPhoneText(text.trim() ? text : null, false)
                setText('')
              })
            }
          >
            {text.trim() ? t('Offer text') : t('Offer what I copied')}
          </button>
        </div>
      )}
      <p className="muted phone-hint">
        {t("Text typed on the phone lands on this computer's clipboard.")}
      </p>
      <div className="phone-actions">
        <button
          className="btn btn-sm"
          onClick={() =>
            act(async (b) => {
              const paths = await b.nearbyPick()
              if (paths.length) await b.nearbyPhoneOffer(paths, null)
            })
          }
        >
          <PaperPlaneTilt size={16} aria-hidden /> {t('Offer files to the phone')}
        </button>
        <button className="btn btn-ghost btn-sm" onClick={() => act((b) => b.nearbyPhone(false))}>
          {t('Stop')}
        </button>
      </div>
    </section>
  )
}

/** Files dragged over the window light up the device under the pointer and go there on drop. */
function useDropOnDevice(): string | null {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [over, setOver] = useState<string | null>(null)
  const paths = useRef<string[]>([])
  useEffect(() => {
    if (!backend) return
    const under = (x: number, y: number) =>
      (document.elementFromPoint(x, y)?.closest('[data-device]') as HTMLElement | null)?.dataset
        .device ?? null
    return backend.onFileDrop((e) => {
      if (e.type === 'leave') {
        setOver(null)
        return
      }
      if (e.paths.length) paths.current = e.paths
      const fp = under(e.x, e.y)
      if (e.type === 'over') setOver(fp)
      else {
        setOver(null)
        if (fp && paths.current.length) {
          const files = paths.current
          void act((b) => b.nearbySend(fp, files))
        }
        paths.current = []
      }
    })
  }, [backend, act])
  return over
}

export function NearbyPanel() {
  const nearby = useApp((s) => s.nearby)
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  // Nearby starts the first time the Send page is shown (NEARBY.md).
  useEffect(() => {
    if (backend && !nearby?.on) void act((b) => b.nearbyStart())
  }, [backend, nearby?.on, act])
  const over = useDropOnDevice()
  const transfers = useMemo(() => nearby?.transfers ?? [], [nearby?.transfers])
  if (!nearby)
    return <div className="sk sk-line" aria-busy="true" aria-label={t('Looking for devices')} />
  return (
    <div className="nearby">
      <div className="nearby-stage">
        <Visibility seconds={nearby.everyoneFor} />
        {nearby.problem && (
          <p className="field-help warn" role="status">
            {nearby.problem}
          </p>
        )}
        <section aria-labelledby="near-devices" className="nearby-devices">
          <h2 className="group" id="near-devices">
            {tr('On this network {n}', {
              n: <span className="num">{nearby.devices.length}</span>,
            })}
          </h2>
          <Radar me={nearby.me} devices={nearby.devices} transfers={transfers} over={over} />
          <p className="send-note muted">
            <LockKey size={14} aria-hidden />{' '}
            {t('Click a device or drop files on it. Files go straight there, encrypted.')}
          </p>
        </section>
      </div>
      <aside className="nearby-side" aria-label={t('Activity')}>
        <section aria-labelledby="near-recent">
          <h2 className="group" id="near-recent">
            {t('Activity')}
          </h2>
          {transfers.length === 0 ? (
            <p className="muted activity-empty">{t('Nothing sent or received yet.')}</p>
          ) : (
            <ul className="activity-list">
              {transfers.map((x) => (
                <Transfer key={x.id} x={x} />
              ))}
            </ul>
          )}
        </section>
        <TextCard devices={nearby.devices} />
        <SyncCard syncs={nearby.syncs} trusted={nearby.trusted} />
        <PhoneCard phone={nearby.phone} />
        {nearby.trusted.length > 0 && (
          <section aria-labelledby="near-trusted">
            <h2 className="group" id="near-trusted">
              {tr('Trusted devices {n}', {
                n: <span className="num">{nearby.trusted.length}</span>,
              })}
            </h2>
            <ul className="trusted">
              {nearby.trusted.map((d) => (
                <li key={d.fingerprint}>
                  <ShieldCheck size={18} aria-hidden />
                  <span>
                    <span className="device-name" translate="no">
                      {d.alias}
                    </span>
                    <br />
                    <span className="muted">
                      {t('Since {date}. Sends without asking.', { date: shortDate(d.since) })}
                    </span>
                  </span>
                  <button
                    className="btn btn-ghost btn-sm"
                    aria-label={t('Forget {name}', { name: d.alias })}
                    onClick={() => act((b) => b.nearbyForget(d.fingerprint))}
                  >
                    {t('Forget')}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </aside>
    </div>
  )
}

/** Someone asks to send files here. Shown on any page; waits for an answer. */
export function NearbyRequestDialog() {
  const req = useApp((s) => s.nearby?.request ?? null)
  const act = useApp((s) => s.act)
  const ref = useRef<HTMLDialogElement>(null)
  const [trust, setTrust] = useState(false)
  const [shown, setShown] = useState<NearbyRequest | null>(null)
  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (req && !d.open) {
      setTrust(false)
      setShown(req)
      d.showModal()
      d.querySelector<HTMLButtonElement>('[data-decline]')?.focus()
    }
    if (!req && d.open) d.close()
  }, [req])
  const r = req ?? shown
  const answer = (accept: boolean) => {
    if (!r) return
    void act((b) => b.nearbyAnswer(r.id, accept, accept && trust))
  }
  return (
    <dialog
      ref={ref}
      className="dialog request-dialog"
      aria-labelledby="nr-title"
      onCancel={(e) => {
        e.preventDefault()
        answer(false)
      }}
    >
      {r && (
        <form onSubmit={(e) => e.preventDefault()}>
          <header className="dialog-head">
            <div className="req-head">
              <span className="device-tile">
                <DeviceIcon kind={r.kind} model={r.model} />
              </span>
              <div>
                <h2 id="nr-title">
                  {r.text !== null
                    ? tr('{name} wants to send you text', {
                        name: <span translate="no">{r.alias}</span>,
                      })
                    : r.files.length === 1
                      ? tr('{name} wants to send you a file', {
                          name: <span translate="no">{r.alias}</span>,
                        })
                      : tr('{name} wants to send you {n} files', {
                          name: <span translate="no">{r.alias}</span>,
                          n: r.files.length,
                        })}
                </h2>
                <p className="muted">
                  {deviceSub({
                    model: r.model,
                    fuselane: /^Fuselane/.test(r.model ?? ''),
                    kind: r.kind,
                  })}
                </p>
              </div>
            </div>
          </header>
          {r.text !== null ? (
            <pre className="req-text" translate="no">
              {r.text}
            </pre>
          ) : (
            <div className="file-summary">
              <span aria-hidden>
                <FolderOpen size={18} />
              </span>
              <span>
                {r.files.length === 1 ? (
                  <b translate="no">{r.files[0]}</b>
                ) : (
                  <b>{tn(r.files.length, '{n} file', '{n} files')}</b>
                )}
                <span className="muted">
                  {tr('{size}, saves to your downloads folder', {
                    size: <span className="num">{bytes(r.total)}</span>,
                  })}
                </span>
              </span>
            </div>
          )}
          <label className="check">
            <input type="checkbox" checked={trust} onChange={(e) => setTrust(e.target.checked)} />
            <span>
              {tr(
                'Trust {name}. Its files arrive without asking next time, and its text goes straight to your clipboard.',
                { name: <span translate="no">{r.alias}</span> },
              )}
            </span>
          </label>
          <footer className="dialog-foot">
            <button
              type="button"
              className="btn btn-ghost"
              data-decline
              onClick={() => answer(false)}
            >
              {t('Decline')}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => answer(true)}>
              {t('Accept')}
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}
