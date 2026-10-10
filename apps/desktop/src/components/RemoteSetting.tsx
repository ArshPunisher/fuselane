import { useEffect, useState } from 'react'
import { Copy, Eye, EyeSlash, QrCode } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import type { RemoteView, UiError } from '../lib/types'

/**
 * Remote control for aria2 apps (8.7, ADR 0013): AriaNg, the Aria2 browser
 * extensions and phone remotes add downloads here and watch them.
 */
export function RemoteSetting() {
  const backend = useApp((s) => s.backend)
  const [view, setView] = useState<RemoteView | null>(null)
  const [port, setPort] = useState('')
  const [shown, setShown] = useState(false)
  const [code, setCode] = useState(false)
  const [error, setError] = useState<UiError | null>(null)
  const [status, setStatus] = useState('')
  useEffect(() => {
    void backend?.remoteState().then((v) => {
      setView(v)
      setPort(String(v.port))
    })
  }, [backend])

  async function run(f: () => Promise<RemoteView>, done = '') {
    setError(null)
    setStatus('')
    try {
      const v = await f()
      setView(v)
      setPort(String(v.port))
      setStatus(done)
    } catch (e) {
      setError(toUiError(e))
    }
  }

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text)
      setStatus(`${what} copied.`)
    } catch {
      setStatus(`Select the ${what.toLowerCase()} and copy it.`)
    }
  }

  const p = Number(port)
  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div>
          <p className="setting-name">Remote control for aria2 apps</p>
          <p className="muted">
            AriaNg, the Aria2 browser extensions and phone remote apps can add downloads here and
            watch them. What they add uses every network, like anything else.
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          className="switch"
          aria-label="Remote control for aria2 apps"
          aria-checked={view?.on === true}
          disabled={view === null}
          onClick={() =>
            view &&
            void run(
              () => backend!.remoteSet(!view.on, view.lan, view.port),
              view.on ? 'Remote control is off.' : '',
            )
          }
        />
      </div>
      {view?.on && (
        <div className="remote">
          {view.problem && (
            <p className="field-error" role="alert">
              {view.problem}
            </p>
          )}
          <dl className="remote-facts">
            <dt>Address</dt>
            {view.urls.map((u) => (
              <dd key={u}>
                <span className="num" translate="no">
                  {u}
                </span>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Copy ${u}`}
                  onClick={() => void copy(u, 'Address')}
                >
                  <Copy size={14} aria-hidden />
                </button>
              </dd>
            ))}
            <dt>Secret</dt>
            <dd>
              <span className="num" translate="no">
                {shown ? view.secret : '•'.repeat(16)}
              </span>
              <button
                type="button"
                className="icon-btn"
                aria-label={shown ? 'Hide the secret' : 'Show the secret'}
                onClick={() => setShown(!shown)}
              >
                {shown ? <EyeSlash size={14} aria-hidden /> : <Eye size={14} aria-hidden />}
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label="Copy the secret"
                onClick={() => void copy(view.secret, 'Secret')}
              >
                <Copy size={14} aria-hidden />
              </button>
              <button
                type="button"
                className="btn btn-sm"
                title="Apps using the old secret stop working until you give them the new one"
                onClick={() => void run(() => backend!.remoteNewSecret(), 'New secret made.')}
              >
                New secret
              </button>
            </dd>
          </dl>
          <p className="field-help">
            In AriaNg: AriaNg Settings, then RPC. Enter the address (WebSocket or HTTP both work)
            and the secret.
          </p>
          <label className="check">
            <input
              type="checkbox"
              checked={view.lan}
              onChange={(e) =>
                void run(() => backend!.remoteSet(true, e.target.checked, view.port))
              }
            />
            <span>
              Allow phones and computers on this network
              <span className="field-help">
                The secret travels unencrypted on the network, so use this only at home or work.
              </span>
            </span>
          </label>
          {view.phoneUrl && view.phoneQr && (
            <div className="remote-phone">
              {code ? (
                <>
                  {/* The SVG is made by the app from the link; nothing from outside goes in. */}
                  <span
                    className="qr"
                    role="img"
                    aria-label="Code to scan with your phone"
                    dangerouslySetInnerHTML={{ __html: view.phoneQr }}
                  />
                  <div>
                    <p className="setting-name">Scan with your phone&apos;s camera</p>
                    <p className="muted">
                      A page opens with your downloads: add links, pause and resume. The code
                      carries the secret, so show it only to your own phone.
                    </p>
                    <button type="button" className="btn btn-sm" onClick={() => setCode(false)}>
                      Hide the code
                    </button>
                  </div>
                </>
              ) : (
                <button type="button" className="btn btn-sm" onClick={() => setCode(true)}>
                  <QrCode size={16} aria-hidden /> Show a code for your phone
                </button>
              )}
            </div>
          )}
          <form
            className="remote-port"
            noValidate
            onSubmit={(e) => {
              e.preventDefault()
              void run(() => backend!.remoteSet(true, view.lan, p), 'Saved.')
            }}
          >
            <div className="field">
              <label htmlFor="remote-port">Port</label>
              <input
                id="remote-port"
                name="port"
                type="number"
                inputMode="numeric"
                min={1024}
                max={65535}
                value={port}
                aria-invalid={error?.code === 'bad-value' ? true : undefined}
                aria-describedby={error ? 'remote-port-err' : 'remote-port-help'}
                onChange={(e) => setPort(e.target.value)}
              />
              {error ? (
                <p id="remote-port-err" className="field-error">
                  {error.message} {error.hint}
                </p>
              ) : (
                <p id="remote-port-help" className="field-help">
                  6800 is what aria2 apps expect.
                </p>
              )}
            </div>
            <button type="submit" className="btn" disabled={p === view.port}>
              Save
            </button>
          </form>
        </div>
      )}
    </div>
  )
}
