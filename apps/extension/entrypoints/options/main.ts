import { DEFAULT_RULES, normalizeRules } from '@fuselane/capture'
import { readForm, toForm, type Field, type FormValues } from '../../lib/settings-form.ts'
import { appStatus, browserId, sendHandoff, settle } from '../../lib/status.ts'
import { SESSION_PERMISSIONS, sessionAllowed } from '../../lib/session.ts'
import './style.css'

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!
const form = $<HTMLFormElement>('#settings')
const enabled = $<HTMLInputElement>('#enabled')
const saved = $<HTMLParagraphElement>('#saved')
const save = $<HTMLButtonElement>('#save')
const FIELDS: Field[] = ['minSize', 'includeDomains', 'excludeDomains', 'extensions', 'mimeTypes']
const input = (f: Field) => $<HTMLInputElement | HTMLTextAreaElement>(`#${f}`)

// What is stored, as the form shows it; anything else is unsaved.
let stored = ''
const dirty = () => JSON.stringify(values()) !== stored

function values(): FormValues {
  const v = { enabled: enabled.checked } as FormValues
  for (const f of FIELDS) v[f] = input(f).value
  return v
}

function fill(v: FormValues) {
  enabled.checked = v.enabled
  for (const f of FIELDS) input(f).value = v[f]
  // Keep a set MIME list visible: it narrows what is taken.
  if (v.mimeTypes) $<HTMLDetailsElement>('#advanced').open = true
}

function showErrors(errors: Partial<Record<Field, string>>) {
  for (const f of FIELDS) {
    const el = $<HTMLParagraphElement>(`#${f}-error`)
    const message = errors[f]
    el.textContent = message ?? ''
    el.hidden = !message
    input(f).toggleAttribute('aria-invalid', Boolean(message))
  }
}

const ask = (m: unknown) => browser.runtime.sendNativeMessage('app.fuselane.host', m as object)

async function load() {
  const got = await browser.storage.local.get(['rules', 'handoffPending'])
  const rules = normalizeRules(got.rules)
  fill(toForm(rules))
  stored = JSON.stringify(values())
  const status = $<HTMLParagraphElement>('#status')
  const s = await appStatus(ask, {
    browser: browserId(navigator.userAgent, 'brave' in navigator),
    version: browser.runtime.getManifest().version,
  })
  status.textContent = s.text
  status.dataset.state = s.state
  // The app decides where downloads go (unless we have a change it hasn't seen):
  // show its setting unless the person has already started editing.
  if (!dirty()) {
    const now = await settle(s, rules, got.handoffPending === true, ask)
    await browser.storage.local.set({ rules: now.rules, handoffPending: now.pending })
    fill(toForm(now.rules))
    stored = JSON.stringify(values())
  }
}

form.addEventListener('submit', async (e) => {
  e.preventDefault()
  const r = readForm(values())
  if (!r.ok) {
    showErrors(r.errors)
    saved.textContent = 'Not saved. Fix the fields marked in red.'
    saved.dataset.state = 'error'
    if (r.errors.mimeTypes) $<HTMLDetailsElement>('#advanced').open = true
    input(FIELDS.find((f) => r.errors[f])!).focus()
    return
  }
  showErrors({})
  save.disabled = true
  save.textContent = 'Saving…'
  try {
    // The app keeps the on/off switch and the size too; tell it, or keep the
    // change waiting for the next time it's running.
    const delivered = await sendHandoff(ask, r.rules)
    await browser.storage.local.set({ rules: r.rules, handoffPending: !delivered })
    // Show what was stored, cleaned: "20mb" comes back as "20 MB".
    fill(toForm(r.rules))
    stored = JSON.stringify(values())
    saved.textContent = 'Saved. New downloads follow these settings.'
    saved.dataset.state = 'ok'
  } catch (err) {
    saved.textContent = `Not saved: the browser refused (${String(err)}). Try again.`
    saved.dataset.state = 'error'
  } finally {
    save.disabled = false
    save.textContent = 'Save settings'
  }
})

$('#reset').addEventListener('click', () => {
  showErrors({})
  fill(toForm(DEFAULT_RULES))
  saved.textContent = 'Defaults restored. Save to keep them.'
  saved.dataset.state = ''
})

// Typing clears a field's error; a stale message is worse than none.
for (const f of FIELDS) {
  input(f).addEventListener('input', () => {
    $<HTMLParagraphElement>(`#${f}-error`).hidden = true
    input(f).removeAttribute('aria-invalid')
  })
}

window.addEventListener('beforeunload', (e) => {
  if (dirty()) e.preventDefault()
})

// Signed-in downloads apply at once: the browser's own permission prompt is the save.
const session = $<HTMLInputElement>('#session')
const sessionStatus = $<HTMLParagraphElement>('#session-status')
void sessionAllowed().then((on) => {
  session.checked = on
})
session.addEventListener('change', async () => {
  try {
    if (session.checked) {
      const granted = await browser.permissions.request(SESSION_PERMISSIONS)
      session.checked = granted
      sessionStatus.textContent = granted
        ? 'On. Downloads that need a sign-in can go to Fuselane now.'
        : 'Not allowed, so downloads that need a sign-in stay in the browser.'
    } else {
      await browser.permissions.remove(SESSION_PERMISSIONS)
      sessionStatus.textContent = 'Off. Fuselane no longer gets cookies.'
    }
  } catch (err) {
    session.checked = await sessionAllowed()
    sessionStatus.textContent = `The browser didn't change the permission (${String(err)}). Try again.`
  }
})

void load()
