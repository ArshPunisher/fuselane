import { useEffect, useId, useRef, useState } from 'react'
import { useApp } from '../lib/store'
import { netTitle, setNetPrefs } from '../lib/lanes'
import { toUiError } from '../lib/backend'
import type { NetPref, NetView, ProxyPref, ProxyType, UiError } from '../lib/types'
import { t, tb } from '../lib/i18n'

const TYPES: { value: ProxyType; label: string; port: string }[] = [
  { value: 'http', label: 'HTTP', port: '8080' },
  { value: 'socks5', label: 'SOCKS5', port: '1080' },
]

/** "SOCKS5 proxy.lan:1080, as ann" */
function summary(p: ProxyPref): string {
  const kind = p.kind === 'http' ? 'HTTP' : 'SOCKS5'
  const at = p.host.includes(':') ? `[${p.host}]:${p.port}` : `${p.host}:${p.port}`
  return p.username
    ? t('{proxy}, as {user}', { proxy: `${kind} ${at}`, user: p.username })
    : `${kind} ${at}`
}

type Check = { state: 'busy' } | { state: 'ok'; text: string } | { state: 'error'; error: UiError }

/** The saved prefs come back from the backend: keep the window's copy in step. */
function keep(prefs: NetPref[]) {
  setNetPrefs(prefs)
  useApp.setState({ netPrefs: prefs })
}

function ProxyForm({
  net,
  saved,
  onDone,
}: {
  net: NetView
  saved: ProxyPref | null
  onDone: (saved: boolean) => void
}) {
  const backend = useApp((s) => s.backend)
  const id = useId()
  const title = netTitle(net)
  const [kind, setKind] = useState<ProxyType>(saved?.kind ?? 'http')
  const [host, setHost] = useState(saved?.host ?? '')
  const [port, setPort] = useState(saved ? String(saved.port) : '')
  const [username, setUsername] = useState(saved?.username ?? '')
  const [password, setPassword] = useState('')
  const [forget, setForget] = useState(false)
  const [touched, setTouched] = useState(false)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState<UiError | null>(null)
  const hostRef = useRef<HTMLInputElement>(null)
  const portRef = useRef<HTMLInputElement>(null)
  useEffect(() => hostRef.current?.focus(), [])

  const portNum = Number(port)
  const portOk = port !== '' && Number.isInteger(portNum) && portNum >= 1 && portNum <= 65535
  const hostError = touched && !host.trim() ? t("Enter the proxy's name or address.") : null
  const portError = touched && !portOk ? t('Use a port from 1 to 65535, like 8080 or 1080.') : null
  const portHint = TYPES.find((type) => type.value === kind)!.port

  const submit = async () => {
    setTouched(true)
    // The first field to fix gets the focus.
    if (!host.trim()) return hostRef.current?.focus()
    if (!portOk) return portRef.current?.focus()
    if (!backend) return
    setBusy(true)
    setFailure(null)
    try {
      const prefs = await backend.setNetworkProxy(net.name, {
        kind,
        host,
        port: portNum,
        username: username.trim() || null,
        // Empty keeps the saved password; forgetting it sends ''.
        ...(password ? { password } : forget ? { password: '' } : {}),
      })
      keep(prefs)
      onDone(true)
    } catch (e) {
      setFailure(toUiError(e))
      setBusy(false)
    }
  }

  const remove = async () => {
    if (!backend) return
    setBusy(true)
    try {
      keep(await backend.setNetworkProxy(net.name, null))
      onDone(false)
    } catch (e) {
      setFailure(toUiError(e))
      setBusy(false)
    }
  }

  return (
    <form
      className="proxy-form"
      aria-label={t('Proxy for {network}', { network: title })}
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        void submit()
      }}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation()
          onDone(false)
        }
      }}
    >
      <div className="proxy-kind">
        <span id={`${id}-kind`} className="proxy-label">
          {t('Type')}
        </span>
        <div
          className="segmented"
          role="radiogroup"
          aria-labelledby={`${id}-kind`}
          onKeyDown={(e) => {
            if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return
            e.preventDefault()
            const next = kind === 'http' ? 'socks5' : 'http'
            setKind(next)
            e.currentTarget.querySelector<HTMLButtonElement>(`[data-kind="${next}"]`)?.focus()
          }}
        >
          {TYPES.map((type) => (
            <button
              key={type.value}
              type="button"
              role="radio"
              data-kind={type.value}
              aria-checked={kind === type.value}
              tabIndex={kind === type.value ? 0 : -1}
              onClick={() => setKind(type.value)}
            >
              {type.label}
            </button>
          ))}
        </div>
      </div>
      <div className="proxy-grid">
        <div className="proxy-where">
          <div className="proxy-field">
            <label htmlFor={`${id}-host`} className="proxy-label">
              {t('Address')}
            </label>
            <input
              ref={hostRef}
              id={`${id}-host`}
              name="proxy-host"
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              // i18n-ignore: an example address
              placeholder="proxy.example.com"
              value={host}
              aria-invalid={hostError ? true : undefined}
              aria-describedby={hostError ? `${id}-host-err` : undefined}
              onChange={(e) => setHost(e.target.value)}
            />
            {hostError && (
              <p id={`${id}-host-err`} className="field-error">
                {hostError}
              </p>
            )}
          </div>
          <div className="proxy-field proxy-port">
            <label htmlFor={`${id}-port`} className="proxy-label">
              {t('Port')}
            </label>
            <input
              ref={portRef}
              id={`${id}-port`}
              name="proxy-port"
              className="num"
              inputMode="numeric"
              autoComplete="off"
              placeholder={portHint}
              value={port}
              aria-invalid={portError ? true : undefined}
              aria-describedby={portError ? `${id}-port-err` : undefined}
              onChange={(e) => setPort(e.target.value.replace(/\D/g, '').slice(0, 5))}
            />
            {portError && (
              <p id={`${id}-port-err`} className="field-error">
                {portError}
              </p>
            )}
          </div>
        </div>
        <div className="proxy-field">
          <label htmlFor={`${id}-user`} className="proxy-label">
            {t('Username')} <span className="muted">{t('(if it asks)')}</span>
          </label>
          <input
            id={`${id}-user`}
            name="proxy-username"
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
        </div>
        <div className="proxy-field">
          <label htmlFor={`${id}-pass`} className="proxy-label">
            {t('Password')} <span className="muted">{t('(if it asks)')}</span>
          </label>
          <input
            id={`${id}-pass`}
            name="proxy-password"
            type="password"
            autoComplete="new-password"
            placeholder={saved?.hasPassword && !forget ? t('Saved') : undefined}
            value={password}
            aria-describedby={saved?.hasPassword ? `${id}-pass-help` : undefined}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
      </div>
      {saved?.hasPassword && (
        <div id={`${id}-pass-help`} className="proxy-saved">
          <p className="field-help">
            {saved.passwordIn === 'keychain'
              ? t(
                  "A password is saved in your system's keychain and never shown. Leave the box empty to keep it.",
                )
              : saved.passwordIn === 'settings'
                ? t(
                    "A password is saved in Fuselane's settings (no system keychain available) and never shown. Leave the box empty to keep it.",
                  )
                : t('A password is saved and never shown. Leave the box empty to keep it.')}
          </p>
          <label className="check">
            <input
              type="checkbox"
              name="proxy-forget-password"
              checked={forget}
              onChange={(e) => setForget(e.target.checked)}
            />
            <span>{t('Forget the saved password')}</span>
          </label>
        </div>
      )}
      {failure && (
        <p className="field-error" role="alert">
          {failure.message}
          {failure.hint ? ` ${failure.hint}` : ''}
        </p>
      )}
      <div className="net-editor-foot">
        {saved && (
          <button
            type="button"
            className="btn btn-ghost proxy-remove"
            disabled={busy}
            onClick={() => void remove()}
          >
            {t('Remove proxy')}
          </button>
        )}
        <button type="button" className="btn btn-ghost" onClick={() => onDone(false)}>
          {t('Cancel')}
        </button>
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? t('Saving…') : t('Save')}
        </button>
      </div>
    </form>
  )
}

function ProxyRow({ net, pref }: { net: NetView; pref: NetPref | undefined }) {
  const backend = useApp((s) => s.backend)
  const [editing, setEditing] = useState(false)
  const [check, setCheck] = useState<Check | null>(null)
  const proxy = pref?.proxy ?? null
  const title = netTitle(net)
  const editRef = useRef<HTMLButtonElement>(null)

  const runCheck = async () => {
    if (!backend) return
    setCheck({ state: 'busy' })
    try {
      setCheck({ state: 'ok', text: await backend.checkNetworkProxy(net.name) })
    } catch (e) {
      setCheck({ state: 'error', error: toUiError(e) })
    }
  }

  return (
    <li className="proxy-row" data-editing={editing || undefined}>
      <div className="proxy-head">
        <span className="proxy-name">
          <span className="net-name" translate="no">
            {title}
          </span>
          <span className="proxy-summary" translate="no">
            {proxy ? summary(proxy) : t('Direct, no proxy')}
          </span>
        </span>
        {!editing && (
          <span className="proxy-actions">
            {proxy && (
              <button
                type="button"
                className="btn btn-sm"
                disabled={check?.state === 'busy'}
                aria-label={t('Check the proxy for {network}', { network: title })}
                onClick={() => void runCheck()}
              >
                {t('Check')}
              </button>
            )}
            <button
              ref={editRef}
              type="button"
              className="btn btn-sm"
              aria-label={
                proxy
                  ? t('Edit the proxy for {network}', { network: title })
                  : t('Set up a proxy for {network}', { network: title })
              }
              onClick={() => {
                setCheck(null)
                setEditing(true)
              }}
            >
              {proxy ? t('Edit') : t('Set up')}
            </button>
          </span>
        )}
      </div>
      {editing && (
        <ProxyForm
          net={net}
          saved={proxy}
          onDone={(saved) => {
            setEditing(false)
            requestAnimationFrame(() => editRef.current?.focus())
            // A saved proxy is checked straight away, so a typo shows now, not mid-download.
            if (saved) void runCheck()
          }}
        />
      )}
      <div className="proxy-status" aria-live="polite" data-state={check?.state}>
        {check?.state === 'busy' && <p className="field-help">{t('Checking the proxy…')}</p>}
        {check?.state === 'ok' && <p className="field-help proxy-ok">{tb(check.text)}</p>}
        {check?.state === 'error' && (
          <p className="field-error">
            {check.error.message}
            {check.error.hint ? ` ${check.error.hint}` : ''}
          </p>
        )}
      </div>
    </li>
  )
}

/**
 * A proxy per network (STEPS 8.4): for a network that only reaches the internet
 * through one, or a site that only answers some addresses.
 */
export function NetworkProxy() {
  const networks = useApp((s) => s.networks).filter((n) => n.usable)
  const prefs = useApp((s) => s.netPrefs)
  if (!networks.length) return null
  return (
    <section className="net-limits net-proxy" aria-labelledby="proxy-title">
      <h2 id="proxy-title" className="section-title">
        {t('Proxy per network')}
      </h2>
      <p className="muted">
        {t(
          'For a network that only reaches the internet through a proxy. Downloads on it go through the proxy, and https stays encrypted end to end: the proxy sees which site, not what you download.',
        )}
      </p>
      <ul className="proxy-list">
        {networks.map((n) => (
          <ProxyRow key={n.name} net={n} pref={prefs.find((p) => p.name === n.name)} />
        ))}
      </ul>
      <p className="field-help">
        {t(
          "Torrents don't use these proxies: they connect to peers directly on each network. Proxy settings from your system aren't used either, only the ones set here.",
        )}
      </p>
    </section>
  )
}
