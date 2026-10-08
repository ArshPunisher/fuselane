import { DEFAULT_RULES, normalizeRules } from '@fuselane/capture'
import { readForm, toForm, type Field, type FormValues } from '../../lib/settings-form.ts'
import { appStatus } from '../../lib/status.ts'
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

async function load() {
  const { rules } = await browser.storage.local.get('rules')
  fill(toForm(normalizeRules(rules)))
  stored = JSON.stringify(values())
  const status = $<HTMLParagraphElement>('#status')
  const s = await appStatus((m) =>
    browser.runtime.sendNativeMessage('app.fuselane.host', m as object),
  )
  status.textContent = s.text
  status.dataset.state = s.state
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
    await browser.storage.local.set({ rules: r.rules })
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

void load()
