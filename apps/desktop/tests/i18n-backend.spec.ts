import { expect, test, type Page } from '@playwright/test'
import { hi } from '../src/locales/hi'

// Text from the core in Hindi (locales/hi-backend.ts): errors and hints, a failed
// download, network notes, feed and watch-folder results. The demo backend sends the
// core's English sentences, so these check the translation where each is shown.

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('favicon')) errors.push(m.text())
  })
  page.on('pageerror', (e) => errors.push(e.message))
  ;(page as Page & { errors: string[] }).errors = errors
})
test.afterEach(async ({ page }) => {
  expect((page as Page & { errors: string[] }).errors).toEqual([])
})

/** The Hindi the screens use for an English label, so selectors follow the catalogue. */
function h(en: string): string {
  const v = hi[en]
  if (!v) throw new Error(`no Hindi for "${en}"`)
  return v
}

/** English words left in `text`, outside names, links, files and addresses. */
function english(text: string): string[] {
  const rest = text
    .replace(/\S*:\/\/\S*|"[^"]*"|\S*[./:]\w+|\w+:/g, ' ')
    .replace(/Fuselane|Wi-Fi|iPhone USB|RSS|Atom|SOCKS5|HTTP|MB|KB/g, ' ')
  return rest.match(/[A-Za-z]{2,}/g) ?? []
}

test('a refused link says why and what to do in Hindi', async ({ page }) => {
  await page.goto('/?empty=1&lang=hi')
  await page
    .getByRole('button', { name: h('New download') })
    .first()
    .click()
  const dialog = page.getByRole('dialog', { name: h('New download') })
  const link = dialog.getByLabel(h('Link'))
  const submit = dialog.getByRole('button', { name: h('Download'), exact: true })
  const error = dialog.locator('.field-error')
  for (const [bad, says] of [
    ['ftp://example.com/file', 'ftp: लिंक नहीं चलते। http:// या https:// लिंक इस्तेमाल करें।'],
    ['not a link', '"not a link" सही लिंक नहीं है।'],
    ['   ', 'डाउनलोड करने के लिए कोई लिंक पेस्ट करें।'],
  ] as const) {
    await link.fill(bad)
    await submit.click()
    await expect(error).toContainText(says)
    await expect(error).toContainText('लिंक http:// या https:// से शुरू होते हैं।')
    expect(english((await error.textContent()) ?? '')).toEqual([])
  }
  // A missing folder, on its own field.
  await link.fill('https://example.com/a.iso')
  await dialog.getByLabel(h('Save to')).fill('relative/nowhere')
  await submit.click()
  await expect(dialog.locator('#nd-dir-err')).toHaveText(
    'फ़ोल्डर "relative/nowhere" मौजूद नहीं है। सेव करने के लिए कोई दूसरा फ़ोल्डर चुनें।',
  )
})

test("a failed download's reason is in Hindi, and follows a language change", async ({ page }) => {
  await page.goto('/?offline=1&lang=hi')
  await page.getByText('podcast-episode-212.mp3').click()
  const alert = page.locator('article.detail .notice[role="alert"]')
  await expect(alert).toContainText(
    'हर नेटवर्क विफल रहा। आखिरी समस्या: कोई भी नेटवर्क सर्वर तक नहीं पहुंच सका।',
  )
  // Switching to English shows the core's own words again, at once.
  await page
    .getByRole('button', { name: h('Settings') })
    .first()
    .click()
  await page
    .getByRole('radiogroup', { name: h('Language') })
    .getByRole('radio', { name: 'English' })
    .click()
  // The download stays selected.
  await page.getByRole('button', { name: 'Downloads' }).first().click()
  await expect(page.locator('article.detail .notice[role="alert"]')).toContainText(
    'Every network failed. Last problem: no network could reach the server.',
  )
})

test("a network's notes and its proxy's refusal are in Hindi", async ({ page }) => {
  await page.goto('/?drop=0&throttle=1&proxytrouble=1&lang=hi')
  const notes = page.getByRole('list', { name: h('Network notes') })
  await expect(notes).toContainText(
    'proxy.office.lan:3128 पर Wi-Fi के प्रॉक्सी ने यूज़रनेम और पासवर्ड नहीं माना। इन्हें नेटवर्क में, प्रॉक्सी के नीचे जांचें।',
  )
  expect(english((await notes.textContent()) ?? '')).toEqual([])
})

test('checking a proxy that turns down the login answers in Hindi', async ({ page }) => {
  await page.goto('/?drop=0&lang=hi')
  await page
    .getByRole('button', { name: h('Networks') })
    .first()
    .click()
  await page
    .getByRole('button', {
      name: h('Set up a proxy for {network}').replace('{network}', 'iPhone USB'),
    })
    .click()
  const form = page.getByRole('form', {
    name: h('Proxy for {network}').replace('{network}', 'iPhone USB'),
  })
  // The service's own check of the address, with its hint.
  await form.getByLabel(h('Address')).fill('http://proxy.office.lan:8080')
  await form.getByLabel(h('Port')).fill('3128')
  await form.getByRole('button', { name: h('Save') }).click()
  await expect(form).toContainText(
    'सिर्फ़ प्रॉक्सी का नाम या पता लिखें। http:// या socks5:// न लिखें, इसकी जगह ऊपर प्रकार चुनें।',
  )
  await form.getByLabel(h('Address')).fill('10.0.0.2')
  await form.getByLabel(h('Username')).fill('ann')
  await form.getByLabel(new RegExp(`^${h('Password')}`)).fill('wrong')
  await form.getByRole('button', { name: h('Save') }).click()
  const row = page.locator('.proxy-row', { hasText: 'iPhone USB' })
  await expect(row.locator('.proxy-status .field-error')).toHaveText(
    '10.0.0.2:3128 पर iPhone USB के प्रॉक्सी ने यूज़रनेम और पासवर्ड नहीं माना। इन्हें नेटवर्क में, प्रॉक्सी के नीचे जांचें।',
  )
  await row
    .getByRole('button', {
      name: h('Edit the proxy for {network}').replace('{network}', 'iPhone USB'),
    })
    .click()
  const edit = page.getByRole('form', {
    name: h('Proxy for {network}').replace('{network}', 'iPhone USB'),
  })
  await edit.getByLabel(new RegExp(`^${h('Password')}`)).fill('right')
  await edit.getByRole('button', { name: h('Save') }).click()
  await expect(row.locator('.proxy-status')).toContainText(
    'काम कर रहा है: iPhone USB इस प्रॉक्सी से इंटरनेट तक पहुंच रहा है।',
  )
})

test('feeds, the watch folder and remote control explain problems in Hindi', async ({ page }) => {
  await page.goto('/?lang=hi')
  // A web page instead of a feed.
  await page.getByRole('button', { name: h('Feeds'), exact: true }).click()
  const feeds = page.getByRole('dialog', { name: h('Feeds') })
  await feeds.getByLabel(h('Feed address')).fill('https://example.com/not-a-feed')
  await feeds.getByRole('button', { name: h('Follow'), exact: true }).click()
  await expect(feeds.locator('#feed-url-err')).toHaveText(
    'यह फ़ीड नहीं है: यह RSS या Atom नहीं है। पता जांचें: यह वेब पेज नहीं, फ़ीड (RSS या Atom) की तरह खुलना चाहिए।',
  )
  await page.keyboard.press('Escape')
  await expect(feeds).toHaveCount(0)

  // What the watch folder did with each file.
  await page
    .getByRole('button', { name: h('Settings') })
    .first()
    .click()
  await page.getByRole('switch', { name: h('Add files from a folder') }).click()
  const taken = page.getByRole('list', { name: h('Files taken from the folder') })
  await expect(taken).toContainText('टोरेंट शुरू हुआ: debian-13.7.0-amd64-netinst.iso।')
  await expect(taken).toContainText('3 डाउनलोड जोड़े गए।')
  await expect(taken).toContainText('नहीं जोड़ा गया: यह सही .torrent फ़ाइल नहीं है।')

  // A port the remote control can't use.
  await page.getByRole('switch', { name: h('Remote control for aria2 apps') }).click()
  const port = page.getByRole('spinbutton', { name: h('Port') })
  await port.fill('80')
  await page
    .getByRole('button', { name: h('Save') })
    .last()
    .click()
  await expect(page.locator('#remote-port-err')).toContainText(
    '1024 से 65535 तक का कोई पोर्ट चुनें।',
  )
})

test('a failed update says why in Hindi', async ({ page }) => {
  await page.goto('/?empty=1&update=bad&lang=hi')
  await page
    .locator('.update-banner')
    .getByRole('button', { name: h('Update and restart') })
    .click()
  const failed = page.getByRole('alert').filter({ hasText: 'अपडेट डाउनलोड नहीं हो सका' })
  await expect(failed).toBeVisible({ timeout: 10_000 })
  await expect(failed).toContainText('अपडेट डाउनलोड नहीं हो सका: हर नेटवर्क 22.0 MB पर टूट गया।')
  await expect(failed).toContainText('यह वहीं से आगे बढ़ेगा जहां रुका था')
})

test('in English, the same messages stay as the core wrote them', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'New download' }).first().click()
  const dialog = page.getByRole('dialog', { name: 'New download' })
  await dialog.getByLabel('Link').fill('not a link')
  await dialog.getByRole('button', { name: 'Download', exact: true }).click()
  await expect(dialog.locator('.field-error')).toHaveText(
    '"not a link" isn\'t a valid link. Links start with http:// or https://.',
  )
})
