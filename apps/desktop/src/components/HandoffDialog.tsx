import { useEffect, useRef, useState } from 'react'
import { Desktop, X } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { JobView, NearbyTransfer } from '../lib/types'
import { bytes } from '../lib/format'
import { t, tr } from '../lib/i18n'

/**
 * Hand a paused download to another Fuselane on this network (B9.9): it shows
 * up there at once and carries on, while what's downloaded here follows. The
 * dialog shows that sending (REVIEW-2026-10-11 C6); it can be closed and the
 * sending goes on (the Send page lists it too).
 */
export function HandoffDialog({
  job,
  open,
  onClose,
}: {
  job: JobView
  open: boolean
  onClose: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const backend = useApp((s) => s.backend)
  const nearby = useApp((s) => s.nearby)
  const [sending, setSending] = useState<string | null>(null)
  const [sent, setSent] = useState<{ id: string; device: string } | null>(null)
  const setView = useApp((s) => s.setView)
  const setSendTab = useApp((s) => s.setSendTab)
  const [problem, setProblem] = useState<string | null>(null)
  const devices = (nearby?.devices ?? []).filter((d) => d.fuselane)

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setSent(null)
      setProblem(null)
      d.showModal()
      // Nearby starts the first time it's needed (NEARBY.md).
      if (backend && !nearby?.on) void backend.nearbyStart().catch(() => {})
    }
    if (!open && d.open) d.close()
  }, [open, backend, nearby?.on])

  const send = async (fingerprint: string, alias: string) => {
    if (!backend) return
    setSending(fingerprint)
    setProblem(null)
    try {
      const id = await backend.nearbyHandoff(job.id, fingerprint)
      setSent({ id, device: alias })
    } catch (e) {
      const u = toUiError(e)
      setProblem([u.message, u.hint].filter(Boolean).join(' '))
    } finally {
      setSending(null)
    }
  }

  return (
    <dialog
      ref={ref}
      className="dialog handoff-dialog"
      aria-labelledby="ho-title"
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      {open && (
        <form className="handoff" method="dialog" onSubmit={(e) => e.preventDefault()}>
          <header className="dialog-head">
            <h2 id="ho-title">{t('Continue on another computer')}</h2>
            <button type="button" className="icon-btn" aria-label={t('Close')} onClick={onClose}>
              <X size={16} aria-hidden />
            </button>
          </header>
          {sent ? (
            <Sending
              transfer={nearby?.transfers.find((x) => x.id === sent.id) ?? null}
              device={sent.device}
              onCancel={(id) => void backend?.nearbyCancel(id).catch(() => {})}
              onShowAll={() => {
                setSendTab('nearby')
                setView('send')
                onClose()
              }}
            />
          ) : (
            <>
              <p className="dialog-text">
                {t(
                  "What's downloaded so far goes along, so the other computer carries on where this one stopped. Both need Fuselane, on the same network.",
                )}
              </p>
              {devices.length ? (
                <ul className="handoff-devices">
                  {devices.map((d) => (
                    <li key={d.fingerprint}>
                      <button
                        type="button"
                        className="btn"
                        disabled={sending !== null}
                        onClick={() => void send(d.fingerprint, d.alias)}
                      >
                        <Desktop size={16} aria-hidden />
                        <span translate="no">{d.alias}</span>
                        {sending === d.fingerprint && (
                          <span className="muted">{t('Sending…')}</span>
                        )}
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className="handoff-help" role="status">
                  <p className="muted">{t('Looking for other computers with Fuselane…')}</p>
                  <p className="dialog-text">{t('On the other computer:')}</p>
                  <ol className="handoff-steps">
                    <li>{t('Open Fuselane, then Send.')}</li>
                    <li>
                      {t(
                        'Under Nearby, choose "Everyone for 10 minutes", or trust this computer when it shows up.',
                      )}
                    </li>
                  </ol>
                  <p className="field-help">
                    {t('Once you trust each other, they always find each other.')}
                  </p>
                </div>
              )}
            </>
          )}
          {problem && (
            <p className="field-error" role="alert">
              {problem}
            </p>
          )}
          <footer className="dialog-foot">
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              {sent ? t('Done') : t('Cancel')}
            </button>
          </footer>
        </form>
      )}
    </dialog>
  )
}

/** The hand-off on its way: how much of what's downloaded has gone (C6). */
function Sending({
  transfer,
  device,
  onCancel,
  onShowAll,
}: {
  transfer: NearbyTransfer | null
  device: string
  onCancel: (id: string) => void
  onShowAll: () => void
}) {
  const who = <b translate="no">{device}</b>
  const going = !transfer || transfer.state === 'asking' || transfer.state === 'sending'
  const pct =
    transfer && transfer.size > 0 ? Math.min(100, (transfer.done / transfer.size) * 100) : 0
  return (
    <div className="handoff-sending">
      <p className="dialog-text" role="status" aria-live="polite">
        {!transfer || transfer.state === 'asking'
          ? tr('Waiting for {device} to take it…', { device: who })
          : transfer.state === 'sending'
            ? tr('Sending {done} of {total} to {device}. It has started there already.', {
                done: <span className="num">{bytes(transfer.done)}</span>,
                total: <span className="num">{bytes(transfer.size)}</span>,
                device: who,
              })
            : transfer.state === 'done'
              ? tr('Everything arrived on {device}. It carries on there; you can remove it here.', {
                  device: who,
                })
              : transfer.state === 'declined'
                ? tr('{device} said no.', { device: who })
                : transfer.state === 'cancelled'
                  ? t(
                      'Sending stopped. What arrived is kept there; it gets the rest from the internet.',
                    )
                  : (transfer.error ?? t("It couldn't be sent."))}
      </p>
      {transfer && transfer.state === 'sending' && (
        <div
          className="bar"
          role="progressbar"
          aria-label={t('Sent')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(pct)}
        >
          <div className="bar-fill" style={{ width: `${pct}%` }} />
        </div>
      )}
      <div className="handoff-actions">
        {going && transfer && (
          <button type="button" className="btn" onClick={() => onCancel(transfer.id)}>
            {t('Stop sending')}
          </button>
        )}
        <button type="button" className="link-btn" onClick={onShowAll}>
          {t('See it in Send')}
        </button>
      </div>
      {going && <p className="field-help">{t('You can close this; sending goes on.')}</p>}
    </div>
  )
}
