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
  X,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes } from '../lib/format'
import type { DeviceView, NearbyRequest, NearbyTransfer, PhoneView } from '../lib/types'

/** "Mac", "Windows", "Linux", or what a LocalSend device says it is. */
function deviceSub(d: Pick<DeviceView, 'model' | 'fuselane' | 'kind'>): string {
  const m = d.model ?? ''
  if (d.fuselane) {
    if (/macos/i.test(m)) return 'Mac'
    if (/windows/i.test(m)) return 'Windows'
    if (/linux/i.test(m)) return 'Linux'
    return 'Fuselane'
  }
  const what = m || (d.kind === 'mobile' ? 'Phone' : 'Computer')
  return `${what}, via LocalSend`
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
    : d.toLocaleDateString([], { day: 'numeric', month: 'short' })
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
    const t = setInterval(() => tick((n) => n + 1), 1000)
    return () => clearInterval(t)
  }, [seconds])
  if (seconds === null) return null
  return Math.max(0, Math.round((until.current - Date.now()) / 1000))
}

const live = (t: NearbyTransfer) =>
  t.state === 'asking' || t.state === 'sending' || t.state === 'receiving'

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
  const active = (d: DeviceView) => transfers.find((t) => t.device === d.alias && live(t))
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
          const t = active(d)
          if (!t) return null
          const p = orbit(devices.length, i)
          const inward = t.direction === 'in'
          return (
            <line
              key={d.fingerprint}
              className="beam"
              data-wait={t.state === 'asking' || undefined}
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
        <span className="radar-me-sub">This computer</span>
      </div>
      {devices.length === 0 && (
        <p className="radar-empty">
          No one here yet. Open Fuselane or LocalSend on the other device, on this Wi-Fi.
        </p>
      )}
      {devices.map((d, i) => {
        const p = orbit(devices.length, i)
        const t = active(d)
        const progress = t && t.state !== 'asking' && t.size ? t.done / t.size : null
        return (
          <button
            key={d.fingerprint}
            type="button"
            className="radar-node"
            data-device={d.fingerprint}
            data-over={over === d.fingerprint || undefined}
            data-busy={t ? t.state : undefined}
            style={{ '--x': `${p.x}%`, '--y': `${p.y}%`, '--i': i } as CSSProperties}
            onClick={() => void sendTo(d.fingerprint)}
            aria-label={`Send files to ${d.alias}`}
            title="Click to choose files, or drop files here"
          >
            <span className="radar-node-tile">
              <Ring value={t ? progress : null} size={64} />
              <DeviceIcon kind={d.kind} model={d.model} />
              {d.trusted && (
                <span className="radar-trust" title="Trusted">
                  <ShieldCheck size={12} weight="fill" aria-hidden />
                </span>
              )}
            </span>
            <span className="radar-node-name" translate="no">
              {d.alias}
            </span>
            <span className="radar-node-sub">
              {t
                ? t.state === 'asking'
                  ? 'Waiting for them…'
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
          Who can see this computer
        </p>
        <p className="muted">
          {everyone && left !== null ? (
            <>
              Everyone here, for{' '}
              <span className="num">
                {Math.floor(left / 60)}:{String(left % 60).padStart(2, '0')}
              </span>{' '}
              more.
            </>
          ) : (
            'Only devices you trust.'
          )}
        </p>
      </div>
      <div className="segmented" role="radiogroup" aria-labelledby="vis-label">
        <button
          type="button"
          role="radio"
          aria-checked={!everyone}
          onClick={() => act((b) => b.nearbySetEveryone(false))}
        >
          Trusted only
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={everyone}
          onClick={() => act((b) => b.nearbySetEveryone(true))}
        >
          Everyone, 10 min
        </button>
      </div>
    </section>
  )
}

const STATE_WORD: Record<NearbyTransfer['state'], string> = {
  asking: 'Waiting for them to accept',
  sending: 'Sending',
  receiving: 'Receiving',
  done: 'Done',
  declined: 'They declined',
  failed: "Didn't arrive",
  cancelled: 'Cancelled',
}

/** One transfer: slides in, fills its ring, turns into a check when done (motion 5). */
function Transfer({ t }: { t: NearbyTransfer }) {
  const act = useApp((s) => s.act)
  const running = live(t)
  const pct = t.size ? Math.min(1, t.done / t.size) : 0
  return (
    <li className="activity" data-state={t.state} data-dir={t.direction}>
      <span className="activity-ic" aria-hidden>
        <Ring
          value={t.state === 'done' ? 1 : running && t.state !== 'asking' ? pct : null}
          size={38}
        />
        {t.state === 'done' ? (
          <Check size={16} weight="bold" className="done-check" />
        ) : t.direction === 'out' ? (
          <PaperPlaneTilt size={16} />
        ) : (
          <DownloadSimple size={16} />
        )}
      </span>
      <span className="activity-text">
        <span className="activity-name" translate="no" title={t.name}>
          {t.name}
        </span>
        <span className="activity-meta">
          {t.direction === 'out' ? 'To' : 'From'} <span translate="no">{t.device}</span>.{' '}
          {t.error ?? STATE_WORD[t.state]}
          {running && t.size > 0 && t.state !== 'asking' && (
            <span className="num">
              {' '}
              {bytes(t.done)} of {bytes(t.size)}
            </span>
          )}
        </span>
        {running && t.state !== 'asking' && (
          <span className="activity-bar" aria-hidden>
            <span style={{ width: `${pct * 100}%` }} />
          </span>
        )}
      </span>
      <span className="activity-actions">
        {running ? (
          <button
            className="btn btn-ghost btn-sm"
            aria-label={`Cancel ${t.name}`}
            onClick={() => act((b) => b.nearbyCancel(t.id))}
          >
            Cancel
          </button>
        ) : (
          <>
            {t.direction === 'in' && t.state === 'done' && t.path && (
              <button className="btn btn-sm" onClick={() => act((b) => b.nearbyReveal(t.id))}>
                <FolderOpen size={16} aria-hidden /> Show
              </button>
            )}
            <button
              className="icon-btn"
              aria-label={`Clear ${t.name}`}
              onClick={() => act((b) => b.nearbyClear(t.id))}
            >
              <X size={16} aria-hidden />
            </button>
          </>
        )}
      </span>
    </li>
  )
}

/** A phone without an app: show a code; it opens a page from this computer (B8.12). */
function PhoneCard({ phone }: { phone: PhoneView | null }) {
  const act = useApp((s) => s.act)
  if (!phone)
    return (
      <section className="phone-card">
        <span className="phone-card-ic" aria-hidden>
          <QrCode size={20} />
        </span>
        <div>
          <p className="setting-name">A phone without Fuselane?</p>
          <p className="muted">It can send and receive in its browser, on this Wi-Fi.</p>
        </div>
        <button className="btn btn-sm" onClick={() => act((b) => b.nearbyPhone(true))}>
          <QrCode size={16} aria-hidden /> Show a code to scan
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
          aria-label="Code to scan with the phone's camera"
          dangerouslySetInnerHTML={{ __html: phone.qr }}
        />
        <div className="phone-text">
          <p className="setting-name">Scan with the phone&apos;s camera</p>
          <p className="muted">
            Or type{' '}
            <span className="num" translate="no">
              {phone.url}
            </span>
          </p>
        </div>
      </div>
      {phone.offers.length > 0 && (
        <ul className="offers" aria-label="Offered to the phone">
          {phone.offers.map((o) => (
            <li key={o.id}>
              <span translate="no">{o.name}</span>
              <span className="num muted">{bytes(o.size)}</span>
              <button
                className="icon-btn"
                aria-label={`Stop offering ${o.name}`}
                onClick={() => act((b) => b.nearbyPhoneOffer([], o.id))}
              >
                <X size={14} aria-hidden />
              </button>
            </li>
          ))}
        </ul>
      )}
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
          <PaperPlaneTilt size={16} aria-hidden /> Offer files to the phone
        </button>
        <button className="btn btn-ghost btn-sm" onClick={() => act((b) => b.nearbyPhone(false))}>
          Stop
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
    return <div className="sk sk-line" aria-busy="true" aria-label="Looking for devices" />
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
            On this network <span className="num">{nearby.devices.length}</span>
          </h2>
          <Radar me={nearby.me} devices={nearby.devices} transfers={transfers} over={over} />
          <p className="send-note muted">
            <LockKey size={14} aria-hidden /> Click a device or drop files on it. Files go straight
            there, encrypted.
          </p>
        </section>
      </div>
      <aside className="nearby-side" aria-label="Activity">
        <section aria-labelledby="near-recent">
          <h2 className="group" id="near-recent">
            Activity
          </h2>
          {transfers.length === 0 ? (
            <p className="muted activity-empty">Nothing sent or received yet.</p>
          ) : (
            <ul className="activity-list">
              {transfers.map((t) => (
                <Transfer key={t.id} t={t} />
              ))}
            </ul>
          )}
        </section>
        <PhoneCard phone={nearby.phone} />
        {nearby.trusted.length > 0 && (
          <section aria-labelledby="near-trusted">
            <h2 className="group" id="near-trusted">
              Trusted devices <span className="num">{nearby.trusted.length}</span>
            </h2>
            <ul className="trusted">
              {nearby.trusted.map((t) => (
                <li key={t.fingerprint}>
                  <ShieldCheck size={18} aria-hidden />
                  <span>
                    <span className="device-name" translate="no">
                      {t.alias}
                    </span>
                    <br />
                    <span className="muted">Since {shortDate(t.since)}. Sends without asking.</span>
                  </span>
                  <button
                    className="btn btn-ghost btn-sm"
                    aria-label={`Forget ${t.alias}`}
                    onClick={() => act((b) => b.nearbyForget(t.fingerprint))}
                  >
                    Forget
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
                  <span translate="no">{r.alias}</span> wants to send you{' '}
                  {r.files.length === 1 ? 'a file' : `${r.files.length} files`}
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
          <div className="file-summary">
            <span aria-hidden>
              <FolderOpen size={18} />
            </span>
            <span>
              <b translate="no">{r.files.length === 1 ? r.files[0] : `${r.files.length} files`}</b>
              <span className="muted num">{bytes(r.total)}, saves to your downloads folder</span>
            </span>
          </div>
          <label className="check">
            <input type="checkbox" checked={trust} onChange={(e) => setTrust(e.target.checked)} />
            <span>
              Trust <span translate="no">{r.alias}</span>. Its files arrive without asking next
              time.
            </span>
          </label>
          <footer className="dialog-foot">
            <button
              type="button"
              className="btn btn-ghost"
              data-decline
              onClick={() => answer(false)}
            >
              Decline
            </button>
            <button type="button" className="btn btn-primary" onClick={() => answer(true)}>
              Accept
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}
