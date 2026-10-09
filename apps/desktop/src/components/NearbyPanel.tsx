import { useEffect, useRef, useState } from 'react'
import {
  DesktopTower,
  DeviceMobile,
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

/** "8:42" counting down every second from the last value the backend gave. */
function useCountdown(seconds: number | null): string | null {
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
  const left = Math.max(0, Math.round((until.current - Date.now()) / 1000))
  return `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`
}

function Words({ words }: { words: string[] }) {
  return (
    <ol className="words" aria-label="Check words">
      {words.map((w) => (
        <li key={w}>{w}</li>
      ))}
    </ol>
  )
}

function DeviceCard({ d, transfer }: { d: DeviceView; transfer: NearbyTransfer | undefined }) {
  const act = useApp((s) => s.act)
  const busy = transfer && (transfer.state === 'asking' || transfer.state === 'sending')
  async function sendFiles() {
    await act(async (b) => {
      const paths = await b.nearbyPick()
      if (paths.length) await b.nearbySend(d.fingerprint, paths)
    })
  }
  return (
    <li className="device" data-state={busy ? 'sending' : undefined}>
      <div className="device-top">
        <span className="device-tile">
          <DeviceIcon kind={d.kind} model={d.model} />
        </span>
        {d.trusted && (
          <span className="device-badge" title="Trusted: sends without asking">
            <ShieldCheck size={12} weight="fill" aria-hidden /> Trusted
          </span>
        )}
      </div>
      <div>
        <p className="device-name" translate="no">
          {d.alias}
        </p>
        <p className="device-sub">{deviceSub(d)}</p>
      </div>
      {busy && transfer ? (
        <div className="device-progress" role="status">
          {transfer.state === 'asking' ? (
            <>
              <span>Waiting for them to accept</span>
              {transfer.words && (
                <>
                  <span className="muted">Their screen shows these words too:</span>
                  <Words words={transfer.words} />
                </>
              )}
            </>
          ) : (
            <>
              <span>
                Sending <b translate="no">{transfer.name}</b>
              </span>
              <div className="bar" data-status="running">
                <div
                  className="bar-fill"
                  style={{ width: `${transfer.size ? (transfer.done / transfer.size) * 100 : 0}%` }}
                />
              </div>
              <span className="num muted">
                {bytes(transfer.done)} of {bytes(transfer.size)}
              </span>
            </>
          )}
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => act((b) => b.nearbyCancel(transfer.id))}
          >
            Cancel
          </button>
        </div>
      ) : (
        <div className="device-foot">
          <button className="btn btn-sm" onClick={() => void sendFiles()}>
            <PaperPlaneTilt size={16} aria-hidden /> Send files
          </button>
        </div>
      )}
    </li>
  )
}

/** A phone without an app: show a code; it opens a page from this computer (B8.12). */
function PhoneCard({ phone }: { phone: PhoneView | null }) {
  const act = useApp((s) => s.act)
  if (!phone)
    return (
      <li className="device device-qr">
        <div className="device-top">
          <span className="device-tile">
            <QrCode size={22} aria-hidden />
          </span>
        </div>
        <div>
          <p className="device-name">A phone without Fuselane?</p>
          <p className="device-sub">It can send and receive in its browser, on this Wi-Fi.</p>
        </div>
        <div className="device-foot">
          <button className="btn btn-sm" onClick={() => act((b) => b.nearbyPhone(true))}>
            <QrCode size={16} aria-hidden /> Show a code to scan
          </button>
        </div>
      </li>
    )
  return (
    <li className="device device-qr phone-on">
      <div className="phone-code">
        {/* The SVG is made by the app from the link; nothing from outside goes in here. */}
        <span
          className="qr"
          role="img"
          aria-label="Code to scan with the phone's camera"
          dangerouslySetInnerHTML={{ __html: phone.qr }}
        />
        <div className="phone-text">
          <p className="device-name">Scan with the phone&apos;s camera</p>
          <p className="device-sub">
            Or type{' '}
            <span className="num" translate="no">
              {phone.url}
            </span>
          </p>
          <p className="device-sub">The page shows these words too:</p>
          <Words words={phone.words} />
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
      <div className="device-foot">
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
    </li>
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

function Transfer({ t }: { t: NearbyTransfer }) {
  const act = useApp((s) => s.act)
  const live = t.state === 'asking' || t.state === 'sending' || t.state === 'receiving'
  return (
    <li className="send-item" data-state={t.state}>
      <div className="send-item-head">
        <span className="send-ic" aria-hidden>
          {t.direction === 'out' ? <PaperPlaneTilt size={18} /> : <FolderOpen size={18} />}
        </span>
        <span className="send-item-text">
          <span className="send-name" translate="no">
            {t.name}
          </span>
          <span className="send-meta">
            {t.direction === 'out' ? 'To' : 'From'} {t.device}. {t.error ?? STATE_WORD[t.state]}
            {live && t.size > 0 && (
              <span className="num">
                {' '}
                {bytes(t.done)} of {bytes(t.size)}
              </span>
            )}
          </span>
        </span>
        {t.direction === 'in' && t.state === 'done' && t.path && (
          <button className="btn btn-sm" onClick={() => act((b) => b.nearbyReveal(t.id))}>
            <FolderOpen size={16} aria-hidden /> Show
          </button>
        )}
        {!live && (
          <button
            className="icon-btn"
            aria-label={`Clear ${t.name}`}
            onClick={() => act((b) => b.nearbyClear(t.id))}
          >
            <X size={16} aria-hidden />
          </button>
        )}
      </div>
    </li>
  )
}

export function NearbyPanel() {
  const nearby = useApp((s) => s.nearby)
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  // Nearby starts the first time the Send page is shown (NEARBY.md).
  useEffect(() => {
    if (backend && !nearby?.on) void act((b) => b.nearbyStart())
  }, [backend, nearby?.on, act])
  const left = useCountdown(nearby?.everyoneFor ?? null)
  if (!nearby)
    return <div className="sk sk-line" aria-busy="true" aria-label="Looking for devices" />
  const everyone = nearby.everyoneFor !== null
  const others = nearby.devices
  return (
    <>
      <section className="visibility" aria-labelledby="vis-label">
        <span className="vis-ic" aria-hidden>
          <Eye size={18} />
        </span>
        <div>
          <p className="setting-name" id="vis-label">
            Who can see this computer
          </p>
          <p className="muted">
            {everyone ? (
              <>
                Everyone on this network, for <span className="num">{left}</span> more. Then only
                trusted devices.
              </>
            ) : (
              'Only trusted devices. Others can’t see it or send to it.'
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
      {nearby.problem && (
        <p className="field-help warn" role="status">
          {nearby.problem}
        </p>
      )}

      <section className="send-section" aria-labelledby="near-devices">
        <h2 className="group" id="near-devices">
          On this network <span className="num">{others.length}</span>
        </h2>
        {others.length === 0 && (
          <p className="muted near-empty">
            No one yet. On the other computer open Fuselane&apos;s Send page; on a phone open
            LocalSend, or show the phone a code below. Both need to be on this Wi-Fi.
          </p>
        )}
        {
          <ul className="device-grid">
            {others.map((d) => (
              <DeviceCard
                key={d.fingerprint}
                d={d}
                transfer={nearby.transfers.find(
                  (t) =>
                    t.direction === 'out' &&
                    t.device === d.alias &&
                    (t.state === 'asking' || t.state === 'sending'),
                )}
              />
            ))}
            <PhoneCard phone={nearby.phone} />
          </ul>
        }
        <p className="send-note muted">
          <LockKey size={14} aria-hidden /> Files go straight to the other device, encrypted. A new
          device shows four words to check before anything is sent.
        </p>
      </section>

      {nearby.transfers.length > 0 && (
        <section className="send-section" aria-labelledby="near-recent">
          <h2 className="group" id="near-recent">
            Recent
          </h2>
          <ul className="send-list">
            {nearby.transfers.map((t) => (
              <Transfer key={t.id} t={t} />
            ))}
          </ul>
        </section>
      )}

      {nearby.trusted.length > 0 && (
        <section className="send-section" aria-labelledby="near-trusted">
          <h2 className="group" id="near-trusted">
            Trusted devices <span className="num">{nearby.trusted.length}</span>
          </h2>
          <ul className="trusted">
            {nearby.trusted.map((t) => (
              <li key={t.fingerprint}>
                <ShieldCheck size={20} aria-hidden />
                <span>
                  <span className="device-name" translate="no">
                    {t.alias}
                  </span>
                  <br />
                  <span className="muted">
                    Trusted since {shortDate(t.since)}. Sends without asking.
                  </span>
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
    </>
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
                  {deviceSub({ model: r.model, fuselane: r.words !== null, kind: r.kind })}
                  {r.verified ? '' : '. Its identity couldn’t be checked'}
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
          {r.words ? (
            <div className="words-check">
              <p className="setting-name">Check that {r.alias} shows the same words</p>
              <Words words={r.words} />
              <p className="muted">
                Different words? Decline. Another device may be pretending to be this one.
              </p>
            </div>
          ) : (
            <p className="field-help">
              This device can&apos;t show check words. Accept only if you expected something from{' '}
              {r.alias}.
            </p>
          )}
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
