import { useEffect, useState } from 'react'
import { useApp, type Theme } from '../lib/store'
import { mark, setLangPref, t, tn, useLang, type LangPref } from '../lib/i18n'
import { toUiError } from '../lib/backend'
import type { SeedSettings, UiError } from '../lib/types'
import { AutoLimit } from './AutoSave'
import { SlowToggle } from './SlowMode'
import { RemoteSetting } from './RemoteSetting'
import { WatchSetting } from './WatchSetting'
import {
  DownloadsAtOnceSetting,
  KeepAwakeSetting,
  LowBatterySetting,
  ScheduleSetting,
  AfterDownloadSetting,
  NameTakenSetting,
  SortSetting,
  WhenDoneSetting,
  WindowSettings,
} from './AutomationSettings'

interface Option<T extends string> {
  id: T
  label: string
  /** The option's own language, when it is written in one (a language's name). */
  lang?: string
}

/** A row of choices that acts as a native radio group (arrow keys move the choice). */
function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string
  options: Option<T>[]
  value: T
  onChange(id: T): void
}) {
  return (
    <div
      className="segmented"
      role="radiogroup"
      aria-label={label}
      onKeyDown={(e) => {
        const i = options.findIndex((o) => o.id === value)
        const step =
          e.key === 'ArrowRight' || e.key === 'ArrowDown'
            ? 1
            : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
              ? -1
              : 0
        if (!step) return
        e.preventDefault()
        const next = options[(i + step + options.length) % options.length]
        if (next) {
          onChange(next.id)
          e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${next.id}"]`)?.focus()
        }
      }}
    >
      {options.map((o) => (
        <button
          key={o.id}
          data-id={o.id}
          role="radio"
          lang={o.lang}
          aria-checked={value === o.id}
          tabIndex={value === o.id ? 0 : -1}
          onClick={() => onChange(o.id)}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}

const THEMES: Option<Theme>[] = [
  { id: 'system', label: mark('System') },
  { id: 'light', label: mark('Light') },
  { id: 'dark', label: mark('Dark') },
]

export function ThemePicker() {
  const theme = useApp((s) => s.theme)
  const setTheme = useApp((s) => s.setTheme)
  return (
    <Segmented
      label={t('Theme')}
      options={THEMES.map((o) => ({ ...o, label: t(o.label) }))}
      value={theme}
      onChange={setTheme}
    />
  )
}

/** Each language is named in itself, so people can find theirs whatever is showing. */
const LANGUAGES: Option<LangPref>[] = [
  { id: 'system', label: mark('System') },
  { id: 'en', label: 'English', lang: 'en' },
  { id: 'hi', label: 'हिन्दी', lang: 'hi' },
]

function LanguageSetting() {
  const pref = useLang((s) => s.pref)
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Language')}</p>
        <p className="muted">{t("System uses your computer's language.")}</p>
      </div>
      <Segmented
        label={t('Language')}
        options={LANGUAGES.map((o) => (o.lang ? o : { ...o, label: t(o.label) }))}
        value={pref}
        onChange={setLangPref}
      />
    </div>
  )
}

function SpeedUnitSetting() {
  const unit = useApp((s) => s.speedUnit)
  const setUnit = useApp((s) => s.setSpeedUnit)
  const options = [
    { id: 'bytes', label: 'MB/s' },
    { id: 'bits', label: 'Mbps' },
  ] as const
  return (
    <div className="setting">
      <div>
        <p className="setting-name" id="unit-label">
          {t('Speed unit')}
        </p>
        <p className="muted">
          {t('MB/s matches file sizes. Mbps matches how internet plans are sold (8 times bigger).')}
        </p>
      </div>
      <div className="segmented" role="radiogroup" aria-labelledby="unit-label">
        {options.map((o) => (
          <button
            key={o.id}
            role="radio"
            aria-checked={unit === o.id}
            tabIndex={unit === o.id ? 0 : -1}
            onClick={() => setUnit(o.id)}
            onKeyDown={(e) => {
              if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) {
                e.preventDefault()
                setUnit(unit === 'bytes' ? 'bits' : 'bytes')
              }
            }}
          >
            {o.label}
          </button>
        ))}
      </div>
    </div>
  )
}

function SpeedLimitSetting() {
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Speed limit')}</p>
        <p className="muted">{t('For all downloads and networks together.')}</p>
      </div>
      <div className="setting-control">
        <AutoLimit
          label={t('Speed limit for all networks')}
          hideLabel
          rate={limits.global}
          save={(global) => save({ ...limits, global })}
          savedText={(r) =>
            r ? t('Saved. Running downloads follow it now.') : t('Limit removed.')
          }
        />
      </div>
    </div>
  )
}

function SlowModeSetting() {
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  return (
    <div className="setting" aria-label={t('Slow mode')} role="group">
      <div>
        <p className="setting-name">{t('Slow mode')}</p>
        <p className="muted">
          {t(
            'One switch for calls and streaming: caps all downloads, then puts your normal limits back.',
          )}
        </p>
      </div>
      <div className="setting-control slow-control">
        <AutoLimit
          label={t('Slow mode speed')}
          hideLabel
          rate={limits.slowRate}
          allowZero={false}
          save={(slowRate) => save({ ...limits, slowRate })}
        />
        <SlowToggle labelled={false} />
      </div>
    </div>
  )
}

const MOD = /Mac/i.test(navigator.platform) ? '⌘' : 'Ctrl'
const SHORTCUTS: [string[], string][] = [
  [[MOD, 'N'], mark('New download (or paste a link anywhere)')],
  [[MOD, '1'], mark('Downloads')],
  [[MOD, '2'], mark('Networks')],
  [[MOD, '3'], mark('Settings')],
  [[MOD, '4'], mark('Send')],
  [[MOD, '5'], mark('Speedtest')],
  [['↑', '↓'], mark('Move through downloads')],
  [['Space'], mark('Pause or resume the selected download')],
  [['Esc'], mark('Back to the list, or close a dialog')],
]

function ShortcutsSetting() {
  return (
    <div className="setting setting-stack">
      <p className="setting-name">{t('Keyboard shortcuts')}</p>
      <dl className="shortcuts">
        {SHORTCUTS.map(([keys, what]) => (
          <div key={what}>
            <dt>
              {keys.map((k) => (
                <kbd key={k}>{k}</kbd>
              ))}
            </dt>
            <dd>{t(what)}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

/** Sharing back after a torrent finishes: off by default, with two stop limits. */
function SharingSetting() {
  const backend = useApp((s) => s.backend)
  const [saved, setSaved] = useState<SeedSettings | null>(null)
  const [ratio, setRatio] = useState('')
  const [minutes, setMinutes] = useState('')
  const [error, setError] = useState<UiError | null>(null)
  const [status, setStatus] = useState('')
  useEffect(() => {
    void backend
      ?.seedSettings()
      .then((s) => {
        setSaved(s)
        setRatio(String(s.ratio))
        setMinutes(String(s.minutes))
      })
      .catch(() => setSaved({ enabled: false, ratio: 1, minutes: 60 }))
  }, [backend])

  async function save(next: SeedSettings) {
    if (!backend) return
    setError(null)
    setStatus('')
    try {
      const s = await backend.setSeedSettings(next)
      setSaved(s)
      setRatio(String(s.ratio))
      setMinutes(String(s.minutes))
      setStatus(s.enabled ? t('Saved.') : t('Sharing is off. Finished torrents stop at once.'))
    } catch (e) {
      setError(toUiError(e))
    }
  }

  const r = Number(ratio)
  const m = Number(minutes)
  const changed = saved !== null && (r !== saved.ratio || m !== saved.minutes)
  const ratioError = error?.code === 'bad-ratio' ? error : null
  const minutesError = error?.code === 'bad-minutes' ? error : null
  return (
    <form
      className="setting setting-stack"
      aria-label={t('Share torrents after downloading')}
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        if (saved) void save({ ...saved, ratio: r, minutes: m })
      }}
    >
      <div className="setting-row">
        <div>
          <p className="setting-name">{t('Share torrents after downloading')}</p>
          <p className="muted">
            {t(
              'Uploads to other people for a while, then stops. Never over a phone tether or cellular while torrents are only sharing.',
            )}
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <button
          type="button"
          role="switch"
          className="switch"
          aria-label={t('Share torrents after downloading')}
          aria-checked={saved?.enabled === true}
          disabled={saved === null}
          onClick={() => saved && void save({ ...saved, enabled: !saved.enabled })}
        />
      </div>
      {saved?.enabled && (
        <div className="share-limits">
          <div className="field">
            <label htmlFor="share-ratio">{t('Stop at ratio')}</label>
            <input
              id="share-ratio"
              name="ratio"
              type="number"
              inputMode="decimal"
              min={0.1}
              max={10}
              step={0.1}
              value={ratio}
              aria-invalid={ratioError ? true : undefined}
              aria-describedby={ratioError ? 'share-ratio-err' : 'share-ratio-help'}
              onChange={(e) => setRatio(e.target.value)}
            />
            {ratioError ? (
              <p id="share-ratio-err" className="field-error">
                {ratioError.message} {ratioError.hint}
              </p>
            ) : (
              <p id="share-ratio-help" className="field-help">
                {t('1 means upload as much as you downloaded.')}
              </p>
            )}
          </div>
          <div className="field">
            <label htmlFor="share-minutes">{t('Stop after (minutes)')}</label>
            <input
              id="share-minutes"
              name="minutes"
              type="number"
              inputMode="numeric"
              min={1}
              max={10080}
              step={1}
              value={minutes}
              aria-invalid={minutesError ? true : undefined}
              aria-describedby={minutesError ? 'share-minutes-err' : 'share-minutes-help'}
              onChange={(e) => setMinutes(e.target.value)}
            />
            {minutesError ? (
              <p id="share-minutes-err" className="field-error">
                {minutesError.message}
              </p>
            ) : (
              <p id="share-minutes-help" className="field-help">
                {t('Whichever limit comes first ends sharing.')}
              </p>
            )}
          </div>
          <button type="submit" className="btn" disabled={!changed}>
            {t('Save')}
          </button>
        </div>
      )}
    </form>
  )
}

function ChecksumSetting() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [on, setOn] = useState<boolean | null>(null)
  useEffect(() => {
    void backend
      ?.findChecksums()
      .then(setOn)
      .catch(() => setOn(true))
  }, [backend])
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Check downloads against published checksums')}</p>
        <p className="muted">
          {t(
            'Many sites put a SHA-256 next to the file (a .sha256 file or SHA256SUMS). Fuselane looks there before it starts and checks the finished file, so a damaged one is never saved.',
          )}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-label={t('Check downloads against published checksums')}
        aria-checked={on === true}
        disabled={on === null}
        onClick={() => void act(async (b) => setOn(await b.setFindChecksums(!on)))}
      />
    </div>
  )
}

function LookupSetting() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [on, setOn] = useState<boolean | null>(null)
  useEffect(() => {
    void backend
      ?.perNetworkDns()
      .then(setOn)
      .catch(() => setOn(false))
  }, [backend])
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Look up servers through each network')}</p>
        <p className="muted">
          {t(
            'Faster when your networks are from different providers: each one gets a server near it. Uses Cloudflare and Google DNS, which then see the names of the sites you download from (never the files).',
          )}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-label={t('Look up servers through each network')}
        aria-checked={on === true}
        disabled={on === null}
        onClick={() => void act(async (b) => setOn(await b.setPerNetworkDns(!on)))}
      />
    </div>
  )
}

function UpdateSetting() {
  const check = useApp((s) => s.checkUpdate)
  const update = useApp((s) => s.update)
  const flatpak = useApp((s) => s.info?.flatpak ?? false)
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  if (flatpak)
    return (
      <div className="setting" data-testid="updates-flatpak">
        <div>
          <p className="setting-name">{t('Updates')}</p>
          <p className="muted">{t('Updates come through your software centre (Flatpak).')}</p>
        </div>
      </div>
    )
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Updates')}</p>
        <p className="muted">
          {t('Fuselane checks a signed update feed. Nothing about you is sent.')}
        </p>
        <p className="muted" role="status">
          {status}
        </p>
      </div>
      <button
        type="button"
        className="btn"
        disabled={busy}
        onClick={async () => {
          setBusy(true)
          setStatus('')
          const r = await check(false)
          setBusy(false)
          if (r === 'current') setStatus(t("You're up to date."))
          if (r === 'available')
            setStatus(
              t('Version {version} is ready to install.', {
                version: useApp.getState().update?.version ?? '',
              }),
            )
        }}
      >
        {busy ? t('Checking…') : update ? t('Check again') : t('Check for updates')}
      </button>
    </div>
  )
}

function ListSetting() {
  const backend = useApp((s) => s.backend)
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  async function run(f: () => Promise<string | null>) {
    if (busy) return
    setBusy(true)
    setStatus('')
    try {
      const said = await f()
      if (said) setStatus(said)
    } catch (e) {
      const err = toUiError(e)
      setStatus(`${err.message}${err.hint ? ` ${err.hint}` : ''}`)
    } finally {
      setBusy(false)
    }
  }
  if (!backend) return null
  return (
    <div className="setting">
      <div>
        <p className="setting-name">{t('Download list')}</p>
        <p className="muted">
          {t(
            'Save every link to a text file, or add links from one. Imported downloads wait until you start them.',
          )}
        </p>
        <p className="muted" role="status">
          {status}
        </p>
      </div>
      <div className="setting-control">
        <button
          type="button"
          className="btn"
          disabled={busy}
          onClick={() =>
            run(async () => {
              const n = await backend.exportLinks()
              return n === null ? null : tn(n, 'Saved 1 link.', 'Saved {n} links.')
            })
          }
        >
          {t('Export…')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy}
          onClick={() =>
            run(async () => {
              const r = await backend.importLinks()
              if (!r) return null
              return r.skipped.length
                ? tn(
                    r.added.length,
                    'Added 1 download; skipped {skipped} already in the list or not valid.',
                    'Added {n} downloads; skipped {skipped} already in the list or not valid.',
                    { skipped: r.skipped.length },
                  )
                : tn(r.added.length, 'Added 1 download.', 'Added {n} downloads.')
            })
          }
        >
          {t('Import…')}
        </button>
      </div>
    </div>
  )
}

function DiagnosticsSetting() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [report, setReport] = useState('')
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  async function copy() {
    if (!backend || busy) return
    setBusy(true)
    setStatus('')
    try {
      const text = await backend.diagnostics()
      setReport(text)
      try {
        await navigator.clipboard.writeText(text)
        setStatus(t('Copied. Paste it into your bug report.'))
      } catch {
        setStatus(t('Select the text below and copy it.'))
      }
    } catch {
      setStatus(t("Couldn't build the report. Try again."))
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="setting setting-stack">
      <div className="setting-row">
        <div>
          <p className="setting-name">{t('Diagnostics')}</p>
          <p className="muted">
            {t(
              'For bug reports. It never includes IP addresses, links or file names, and Fuselane sends nothing by itself. Report a problem opens a GitHub issue with it filled in.',
            )}
          </p>
          <p className="muted" role="status">
            {status}
          </p>
        </div>
        <div className="setting-control">
          <button type="button" className="btn" onClick={copy} disabled={busy}>
            {busy ? t('Collecting…') : t('Copy diagnostics')}
          </button>
          <button
            type="button"
            className="btn"
            title={t(
              'Opens a bug report on GitHub with the diagnostics filled in; you read it before sending',
            )}
            onClick={() => void act((b) => b.reportProblem(null))}
          >
            {t('Report a problem')}
          </button>
        </div>
      </div>
      {report && (
        <textarea
          className="report"
          readOnly
          value={report}
          aria-label={t('Diagnostics report')}
          spellCheck={false}
          rows={10}
        />
      )}
    </div>
  )
}

/** A titled group of settings; groups sit side by side on wide windows. */
function Group({
  id: name,
  title,
  children,
}: {
  id: string
  title: string
  children: React.ReactNode
}) {
  // A fixed id: one made from the (translated) title would repeat in Hindi.
  const id = `set-${name}`
  return (
    <section className="settings-group" aria-labelledby={id}>
      <h2 className="group" id={id}>
        {title}
      </h2>
      {children}
    </section>
  )
}

export function SettingsView() {
  const info = useApp((s) => s.info)
  const demo = useApp((s) => s.backend?.demo)
  return (
    <section className="page" aria-labelledby="set-title">
      <header className="page-head">
        <h1 id="set-title">{t('Settings')}</h1>
      </header>
      <div className="settings-groups">
        <Group id="look-and-feel" title={t('Look and feel')}>
          <div className="setting">
            <div>
              <p className="setting-name">{t('Appearance')}</p>
              <p className="muted">{t('Follows your system unless you pick one.')}</p>
            </div>
            <ThemePicker />
          </div>
          <LanguageSetting />
          <SpeedUnitSetting />
        </Group>
        <Group id="speed" title={t('Speed')}>
          <SpeedLimitSetting />
          <DownloadsAtOnceSetting />
          <SlowModeSetting />
          <ScheduleSetting />
        </Group>
        <Group id="files" title={t('Files')}>
          <div className="setting">
            <div>
              <p className="setting-name">{t('Downloads folder')}</p>
              <p className="muted num" translate="no">
                {info?.defaultDir ?? ''}
              </p>
            </div>
          </div>
          <SortSetting />
          <AfterDownloadSetting />
          <NameTakenSetting />
          <ChecksumSetting />
          <WatchSetting />
          <ListSetting />
        </Group>
        <Group id="this-computer" title={t('This computer')}>
          <WhenDoneSetting />
          <KeepAwakeSetting />
          <LowBatterySetting />
          <WindowSettings />
        </Group>
        <Group id="torrents-and-lookups" title={t('Torrents and lookups')}>
          <SharingSetting />
          <LookupSetting />
        </Group>
        <Group id="other-apps" title={t('Other apps')}>
          {info?.flatpak && (
            <div className="setting" data-testid="extension-flatpak">
              <div>
                <p className="setting-name">{t('Browser extension')}</p>
                <p className="muted">
                  {t(
                    "The browser extension can't talk to the Flatpak version yet; use the .deb or AppImage for it.",
                  )}
                </p>
              </div>
            </div>
          )}
          <RemoteSetting />
        </Group>
        <Group id="about" title={t('About')}>
          <UpdateSetting />
          <div className="setting">
            <div>
              <p className="setting-name">{t('Welcome')}</p>
              <p className="muted">
                {t('The short tour from the first launch: networks, a check, tips.')}
              </p>
            </div>
            <button
              type="button"
              className="btn"
              onClick={() => useApp.setState({ welcomeOpen: true })}
            >
              {t('Show it again')}
            </button>
          </div>
          <ShortcutsSetting />
          <DiagnosticsSetting />
          <div className="setting">
            <div>
              <p className="setting-name">{t('Version')}</p>
              <p className="muted num">
                {demo
                  ? t('{version}, demo data', { version: info?.version ?? '' })
                  : (info?.version ?? '')}
              </p>
            </div>
          </div>
        </Group>
      </div>
    </section>
  )
}
