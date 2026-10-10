import { useEffect, useRef, useState } from 'react'
import { ArrowRight, Clipboard, FolderSimple, Gauge, PuzzlePiece } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import type { CheckRun } from '../lib/types'
import { NetIcon } from './NetIcon'

type Step = 'networks' | 'check' | 'ready'
const STEPS: Step[] = ['networks', 'check', 'ready']

const OS = /Mac/i.test(navigator.platform)
  ? 'mac'
  : /Win/i.test(navigator.platform)
    ? 'win'
    : 'linux'
const MOD = OS === 'mac' ? '⌘' : 'Ctrl'

/** Why a phone or second network may not show up, for this system (NETWORKING.md). */
function NetworkHelp() {
  return (
    <details className="welcome-more">
      <summary>Phone or second network not showing up?</summary>
      <ul>
        {OS === 'mac' ? (
          <>
            <li>
              <b>iPhone:</b> turn on Personal Hotspot, then connect it with a USB cable. It shows up
              as iPhone USB.
            </li>
            <li>
              <b>Android:</b> macOS can&apos;t use Android USB tethering by itself. It needs a
              third-party driver such as TetherKit. Joining the phone&apos;s Wi‑Fi hotspot instead
              replaces your Wi‑Fi, so it doesn&apos;t add a network.
            </li>
          </>
        ) : (
          <li>
            <b>Phone:</b> connect it with a USB cable and turn on USB tethering (Android: Settings,
            Network, Hotspot and tethering; iPhone: Personal Hotspot).
          </li>
        )}
        {OS === 'win' && (
          <li>
            <b>Wi‑Fi turns off when Ethernet is plugged in?</b> Windows does that to save
            connections. In the Group Policy editor, open Computer Configuration, Administrative
            Templates, Network, Windows Connection Manager, and set &quot;Minimize the number of
            simultaneous connections to the Internet or a Windows Domain&quot; to Disabled. On
            Windows Home, set the registry value <code>fMinimizeConnections</code> to 0 under{' '}
            <code>HKLM\SOFTWARE\Policies\Microsoft\Windows\WcmSvc\GroupPolicy</code>.
          </li>
        )}
        <li>
          <b>Wi‑Fi and Ethernet together:</b> connect both. A network joins as soon as it has an
          address; no restart needed.
        </li>
      </ul>
    </details>
  )
}
const mbps = (bps: number) => `${Math.round((bps * 8) / 1e6)} Mbps`

/** One honest sentence about a finished check. */
export function verdict(run: CheckRun): string {
  const ok = run.results.filter((r) => r.downBps !== null && r.downBps > 0)
  if (!ok.length)
    return "The check couldn't reach the test server. You can run it later from Networks."
  const best = ok.reduce((a, b) => ((b.downBps ?? 0) > (a.downBps ?? 0) ? b : a))
  const top = best.downBps ?? 0
  const together = run.togetherBps ?? 0
  if (ok.length < 2 || together <= 0) return `${best.label}: ${mbps(top)}.`
  const times = together / top
  if (times < 1.1)
    return `Together: ${mbps(together)}, about the same as ${best.label} alone. One connection is likely the limit, not the networks.`
  return `Together: ${mbps(together)}, ${times.toFixed(1)} times ${best.label} alone (${mbps(top)}).`
}

/**
 * The welcome (guided first run): what Fuselane found, an optional check of how
 * fast the networks are together, and where to start. Shown once; Settings can
 * show it again.
 */
export function Welcome() {
  const ref = useRef<HTMLDialogElement>(null)
  const backend = useApp((s) => s.backend)
  const open = useApp((s) => s.welcomeOpen)
  const networks = useApp((s) => s.networks)
  const check = useApp((s) => s.netCheck)
  const info = useApp((s) => s.info)
  const act = useApp((s) => s.act)
  const [step, setStep] = useState<Step>('networks')
  const [started, setStarted] = useState(false)

  // A fresh install opens it by itself.
  useEffect(() => {
    if (!backend) return
    void backend
      .welcomeSeen()
      .then((seen) => !seen && useApp.setState({ welcomeOpen: true }))
      .catch(() => {})
  }, [backend])

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setStep('networks')
      setStarted(false)
      d.showModal()
    }
    if (!open && d.open) d.close()
  }, [open])

  // The primary action takes focus when it opens and on each step (not Skip).
  useEffect(() => {
    if (open) ref.current?.querySelector<HTMLButtonElement>('.btn-primary')?.focus()
  }, [open, step])

  const close = () => {
    useApp.setState({ welcomeOpen: false })
    void backend?.setWelcomeSeen(true).catch(() => {})
  }

  const usable = networks.filter((n) => n.usable)
  const run = check?.running ? check.current : started ? (check?.history[0] ?? null) : null
  const index = STEPS.indexOf(step)
  const next = () => setStep(STEPS[Math.min(index + 1, STEPS.length - 1)]!)

  return (
    <dialog
      ref={ref}
      className="dialog welcome"
      aria-labelledby="welcome-title"
      onCancel={(e) => {
        e.preventDefault()
        close()
      }}
    >
      {open && (
        <div className="welcome-body">
          <div
            className="welcome-steps"
            role="progressbar"
            aria-label="Welcome"
            aria-valuemin={1}
            aria-valuemax={STEPS.length}
            aria-valuenow={index + 1}
            aria-valuetext={`Step ${index + 1} of ${STEPS.length}`}
          >
            {STEPS.map((s, i) => (
              <span key={s} data-done={i <= index} />
            ))}
          </div>

          {step === 'networks' && (
            <section className="welcome-step" key="networks">
              <h2 id="welcome-title">Welcome to Fuselane</h2>
              <p className="welcome-lead">
                Fuselane splits each download across every network this computer has, then joins the
                parts into one file.
              </p>
              <ul className="welcome-nets" aria-label="Networks found">
                {usable.map((n) => (
                  <li key={n.name}>
                    <span className="welcome-net-ic">
                      <NetIcon kind={n.kind} />
                    </span>
                    <span className="welcome-net-name" translate="no">
                      {n.label}
                    </span>
                    <span className="muted">
                      {n.reach === 'offline'
                        ? 'No internet'
                        : n.reach === 'portal'
                          ? 'Needs a sign-in page'
                          : 'Ready'}
                    </span>
                  </li>
                ))}
                {!usable.length && <li className="muted">Looking for networks…</li>}
              </ul>
              {usable.length > 0 && <NetworkHelp />}
              {usable.length === 1 && (
                <p className="welcome-hint">
                  One network so far. To go faster, plug in your phone with a USB cable and turn on
                  USB tethering (Personal Hotspot on iPhone), or use Wi‑Fi and Ethernet together.
                  New networks join by themselves.
                </p>
              )}
            </section>
          )}

          {step === 'check' && (
            <section className="welcome-step" key="check">
              <h2 id="welcome-title">How fast are they together?</h2>
              <p className="welcome-lead">
                A quick check measures each network, then all of them at once. It takes about a
                minute and uses roughly 25 MB per network, so skip it on a tight data plan.
              </p>
              {run && run.results.length > 0 && (
                <ul className="welcome-results" aria-label="Check results">
                  {run.results.map((r) => (
                    <li key={r.name}>
                      <span className="welcome-net-ic">
                        <NetIcon kind={r.kind} />
                      </span>
                      <span className="welcome-net-name" translate="no">
                        {r.label}
                      </span>
                      <span className="num">
                        {r.downBps ? mbps(r.downBps) : (r.problem ?? 'No answer')}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              <p className="welcome-verdict" role="status">
                {check?.running ? (check.phase ?? 'Checking…') : run && started ? verdict(run) : ''}
              </p>
              {!check?.running && !started && (
                <button
                  type="button"
                  className="btn"
                  onClick={() => {
                    setStarted(true)
                    void act(async (b) => useApp.setState({ netCheck: await b.netCheckStart() }))
                  }}
                >
                  <Gauge size={16} aria-hidden /> Run the check
                </button>
              )}
            </section>
          )}

          {step === 'ready' && (
            <section className="welcome-step" key="ready">
              <h2 id="welcome-title">Ready when you are</h2>
              <ul className="welcome-tips">
                <li>
                  <Clipboard size={20} aria-hidden />
                  <span>
                    <b>Paste a link anywhere</b> in the window, or press <kbd>{MOD}</kbd>{' '}
                    <kbd>N</kbd>.
                  </span>
                </li>
                <li>
                  <PuzzlePiece size={20} aria-hidden />
                  <span>
                    <b>The browser extension</b> hands big downloads to Fuselane by itself. Get it
                    from fuselane.app.
                  </span>
                </li>
                <li>
                  <FolderSimple size={20} aria-hidden />
                  <span>
                    <b>Files go to</b>{' '}
                    <span className="num" translate="no">
                      {info?.defaultDir ?? 'your Downloads folder'}
                    </span>
                    . Change it for any download.
                  </span>
                </li>
              </ul>
            </section>
          )}

          <footer className="dialog-foot">
            {step !== 'ready' && (
              <button type="button" className="btn btn-ghost foot-start" onClick={close}>
                Skip
              </button>
            )}
            {step === 'ready' ? (
              <button type="button" className="btn btn-primary" onClick={close}>
                Start downloading
              </button>
            ) : (
              <button type="button" className="btn btn-primary" onClick={next}>
                Next <ArrowRight size={16} aria-hidden />
              </button>
            )}
          </footer>
        </div>
      )}
    </dialog>
  )
}
