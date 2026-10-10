import { useEffect, useId, useRef, useState } from 'react'
import {
  ArrowDown,
  CheckCircle,
  Copy,
  FolderOpen,
  LockKey,
  PaperPlaneTilt,
  WarningCircle,
  X,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import { bytes } from '../lib/format'
import type { ReceiveView, ShareView, UiError } from '../lib/types'
import { SEND_PAGE } from '../lib/sendLink'
import { NearbyPanel } from './NearbyPanel'
import { mark, t, tn } from '../lib/i18n'

function shareStatus(s: ShareView): string {
  switch (s.state) {
    case 'preparing':
      return t('Preparing… {pct}%', { pct: Math.floor(s.prepared * 100) })
    case 'sharing': {
      const who =
        s.peers === 0
          ? t('Waiting for the receiver')
          : tn(s.peers, '1 receiver connected', '{n} receivers connected')
      return s.sent > 0 ? t('{who} · {size} sent', { who, size: bytes(s.sent) }) : who
    }
    case 'sent':
      return t('Sent in full. Sharing stopped')
    case 'changed':
      return t('File changed')
    case 'failed':
      return t("Couldn't share")
  }
}

function CopyLink({ link, name }: { link: string; name: string }) {
  const [copied, setCopied] = useState(false)
  const field = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!copied) return
    const timer = setTimeout(() => setCopied(false), 1800)
    return () => clearTimeout(timer)
  }, [copied])
  async function copy() {
    try {
      await navigator.clipboard.writeText(link)
      setCopied(true)
    } catch {
      // No clipboard access: select it so Cmd/Ctrl+C works.
      field.current?.select()
    }
  }
  return (
    <div className="send-link">
      <input
        ref={field}
        readOnly
        value={link}
        aria-label={t('Link for {name}', { name })}
        onFocus={(e) => e.currentTarget.select()}
        spellCheck={false}
      />
      <button type="button" className="btn btn-primary" onClick={copy}>
        {copied ? (
          <CheckCircle size={16} aria-hidden weight="fill" />
        ) : (
          <Copy size={16} aria-hidden />
        )}
        {copied ? t('Copied') : t('Copy link')}
      </button>
    </div>
  )
}

function ShareRow({ s }: { s: ShareView }) {
  const act = useApp((st) => st.act)
  return (
    <li className="send-item" data-state={s.state}>
      <div className="send-item-head">
        <PaperPlaneTilt size={18} aria-hidden className="send-ic" />
        <div className="send-item-text">
          <p className="send-name" title={s.name} translate="no">
            {s.name}
          </p>
          <p className="send-meta">
            <span className="num">{bytes(s.size)}</span>
            <span aria-hidden> · </span>
            <span className="send-state" aria-live="polite">
              {shareStatus(s)}
            </span>
          </p>
        </div>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => void act((b) => b.stopSend(s.id))}
          disabled={s.state === 'preparing'}
          aria-label={t('Stop sending {name}', { name: s.name })}
        >
          {s.state === 'sharing' ? t('Stop') : t('Remove')}
        </button>
      </div>
      {s.state === 'preparing' && (
        <div
          className="bar"
          data-status="running"
          role="progressbar"
          aria-label={t('Preparing {name}', { name: s.name })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.floor(s.prepared * 100)}
        >
          <div className="bar-fill" style={{ width: `${s.prepared * 100}%` }} />
        </div>
      )}
      {s.link && <CopyLink link={s.link} name={s.name} />}
      {s.state === 'sharing' && (
        <label className="send-once">
          <input
            type="checkbox"
            checked={s.once}
            onChange={(e) => void act((b) => b.sendOnce(s.id, e.target.checked))}
          />
          {t('Stop sharing after one full copy is sent')}
        </label>
      )}
      {s.error && (
        <p className="send-error">
          <WarningCircle size={14} aria-hidden weight="fill" /> {s.error}
        </p>
      )}
    </li>
  )
}

function receiveStatus(r: ReceiveView): string {
  switch (r.state) {
    case 'finding':
      return t('Looking for the sender…')
    case 'receiving':
      return r.size
        ? t('{done} of {total}', { done: bytes(r.done), total: bytes(r.size) })
        : t('Receiving…')
    case 'checking':
      return t('Checking it arrived whole…')
    case 'done':
      return t('Arrived and checked')
    case 'failed':
      return t("Didn't arrive")
  }
}

function ReceiveRow({ r }: { r: ReceiveView }) {
  const act = useApp((st) => st.act)
  const pct = r.size ? Math.min(100, (r.done / r.size) * 100) : 0
  const ended = r.state === 'done' || r.state === 'failed'
  return (
    <li className="send-item" data-state={r.state}>
      <div className="send-item-head">
        {r.state === 'done' ? (
          <CheckCircle size={18} aria-hidden weight="fill" className="send-ic ic-success" />
        ) : r.state === 'failed' ? (
          <WarningCircle size={18} aria-hidden weight="fill" className="send-ic ic-danger" />
        ) : (
          <ArrowDown size={18} aria-hidden className="send-ic" />
        )}
        <div className="send-item-text">
          <p className="send-name" title={r.name} translate="no">
            {r.name}
          </p>
          <p className="send-meta">
            <span className="send-state" aria-live="polite">
              {receiveStatus(r)}
            </span>
          </p>
        </div>
        {r.state === 'done' && (
          <button
            type="button"
            className="btn"
            onClick={() => void act((b) => b.revealReceived(r.id))}
          >
            <FolderOpen size={16} aria-hidden />
            {t('Show')}
          </button>
        )}
        {ended && (
          <button
            type="button"
            className="icon-btn"
            aria-label={t('Remove {name} from the list', { name: r.name })}
            onClick={() => void act((b) => b.dismissReceive(r.id))}
          >
            <X size={16} aria-hidden />
          </button>
        )}
      </div>
      {(r.state === 'receiving' || r.state === 'checking' || r.state === 'finding') && (
        <div
          className="bar"
          data-status="running"
          data-indeterminate={r.state === 'finding' ? '' : undefined}
          role="progressbar"
          aria-label={t('Receiving {name}', { name: r.name })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={r.state === 'finding' ? undefined : Math.floor(pct)}
        >
          <div className="bar-fill" style={{ width: r.state === 'finding' ? '30%' : `${pct}%` }} />
        </div>
      )}
      {r.error && (
        <p className="send-error">
          <WarningCircle size={14} aria-hidden weight="fill" /> {r.error}
        </p>
      )}
    </li>
  )
}

function ReceiveForm() {
  const backend = useApp((s) => s.backend)
  const draft = useApp((s) => s.receiveDraft)
  const [link, setLink] = useState(draft)
  const [error, setError] = useState<UiError | null>(null)
  const [busy, setBusy] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const id = useId()

  // A link pasted or dropped elsewhere lands here.
  useEffect(() => {
    if (!draft) return
    setLink(draft)
    setError(null)
    useApp.setState({ receiveDraft: '' })
    input.current?.focus()
  }, [draft])

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!backend) return
    const text = link.trim()
    if (!text) {
      setError({ code: 'empty', message: t('Paste the link someone sent you.'), hint: null })
      input.current?.focus()
      return
    }
    setBusy(true)
    try {
      await backend.receiveLink(text, null)
      setLink('')
      setError(null)
    } catch (err) {
      setError(toUiError(err))
      input.current?.focus()
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="field send-receive" onSubmit={submit} noValidate>
      <label htmlFor={id}>{t('Link someone sent you')}</label>
      <div className="field-row">
        <input
          ref={input}
          id={id}
          name="link"
          value={link}
          onChange={(e) => {
            setLink(e.target.value)
            setError(null)
          }}
          placeholder={`${SEND_PAGE}v1.…`}
          autoComplete="off"
          spellCheck={false}
          aria-invalid={error ? 'true' : undefined}
          aria-describedby={error ? `${id}-error` : `${id}-help`}
        />
        <button type="submit" className="btn btn-primary" disabled={busy}>
          <ArrowDown size={16} aria-hidden weight="bold" />
          {busy ? t('Opening…') : t('Receive')}
        </button>
      </div>
      {error ? (
        <p className="field-error" id={`${id}-error`} role="alert">
          {error.message}
          {error.hint ? ` ${error.hint}` : ''}
        </p>
      ) : (
        <p className="field-help" id={`${id}-help`}>
          {t('Saved to your download folder and checked against what the sender shared.')}
        </p>
      )}
    </form>
  )
}

export function SendView() {
  const shares = useApp((s) => s.shares)
  const receives = useApp((s) => s.receives)
  const act = useApp((s) => s.act)
  const backend = useApp((s) => s.backend)

  async function choose() {
    if (!backend) return
    await act(async (b) => {
      const path = await b.pickSendFile()
      if (path) await b.sendFile(path)
    })
  }

  const tab = useApp((s) => s.sendTab)
  const setTab = useApp((s) => s.setSendTab)
  const tabs: ['nearby' | 'link', string][] = [
    ['nearby', mark('Nearby')],
    ['link', mark('Link')],
  ]
  return (
    <div className="page send-page">
      <div className="page-head with-tabs">
        <h1>{t('Send')}</h1>
        <div
          className="segmented"
          role="radiogroup"
          aria-label={t('How to send')}
          onKeyDown={(e) => {
            if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
              e.preventDefault()
              const next = tab === 'nearby' ? 'link' : 'nearby'
              setTab(next)
              e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${next}"]`)?.focus()
            }
          }}
        >
          {tabs.map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="radio"
              data-id={id}
              aria-checked={tab === id}
              tabIndex={tab === id ? 0 : -1}
              onClick={() => setTab(id)}
            >
              {t(label)}
            </button>
          ))}
        </div>
      </div>
      {tab === 'nearby' ? (
        <>
          <p className="page-lead muted">
            {t('Send to computers and phones on this network. Nothing goes through the internet.')}
          </p>
          <NearbyPanel />
        </>
      ) : (
        <>
          <p className="page-lead muted">
            {t(
              'Send a file of any size straight from this computer. No upload, no account, no size limit: the receiver gets it directly from you, encrypted, and only the link can open it.',
            )}
          </p>

          <section className="send-section" aria-labelledby="send-out">
            <h2 className="group" id="send-out">
              {t('Send a file')}
            </h2>
            <button type="button" className="send-drop" onClick={() => void choose()}>
              <span className="send-drop-orb" aria-hidden>
                <PaperPlaneTilt size={26} weight="duotone" />
              </span>
              <span className="send-drop-text">
                <span className="send-drop-title">{t('Choose a file to send')}</span>
                <span className="muted">
                  {t('You get a link to give to the person receiving it.')}
                </span>
              </span>
            </button>
            <p className="send-note muted">
              <LockKey size={14} aria-hidden />{' '}
              {t(
                "Keep Fuselane open until it arrives. On the same network it connects straight away; across the internet your router needs UPnP on. The key is only in the link, so share it the way you'd share a password.",
              )}
            </p>
            {shares.length > 0 && (
              <ul className="send-list" aria-label={t("Files you're sending")}>
                {shares.map((s) => (
                  <ShareRow key={s.id} s={s} />
                ))}
              </ul>
            )}
          </section>

          <section className="send-section" aria-labelledby="send-in">
            <h2 className="group" id="send-in">
              {t('Receive')}
            </h2>
            <ReceiveForm />
            {receives.length > 0 && (
              <ul className="send-list" aria-label={t("Files you're receiving")}>
                {receives.map((r) => (
                  <ReceiveRow key={r.id} r={r} />
                ))}
              </ul>
            )}
          </section>
        </>
      )}
    </div>
  )
}
