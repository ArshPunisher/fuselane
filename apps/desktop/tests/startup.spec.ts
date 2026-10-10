import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'

// The window that opens instead of the app when the download list can't be opened
// at launch (src-tauri/src/startup.rs; ?startup= in lib/demoStartup.ts). Before it,
// the app exited with a line on stderr nobody launching from Finder could see.

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

const action = (page: Page) => page.locator('html').getAttribute('data-demo-action')

async function noSideways(page: Page) {
  const wide = await page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
  )
  expect(wide).toBe(false)
}

for (const [width, height] of [
  [1440, 900],
  [375, 812],
] as const) {
  test.describe(`at ${width} px`, () => {
    test.use({ viewport: { width, height } })

    test('a list from a newer Fuselane offers the update, which downloads and installs', async ({
      page,
    }) => {
      await page.goto('/?startup=newer&update=1')
      await expect(
        page.getByRole('heading', { name: 'Your download list is from a newer Fuselane' }),
      ).toBeVisible()
      await expect(page.getByText('This copy is version 0.1.0-beta.10.')).toBeVisible()
      // Only the problem screen: none of the app.
      await expect(page.getByRole('navigation')).toHaveCount(0)
      await expect(page.getByRole('button', { name: 'New download' })).toHaveCount(0)
      await noSideways(page)
      await page.getByRole('button', { name: 'Update now' }).click()
      await expect(page.getByText('Downloading Fuselane 0.1.0-beta.11')).toBeVisible()
      await expect(page.getByRole('progressbar', { name: 'Update download' })).toBeVisible()
      await expect(page.getByText('Installing 0.1.0-beta.11.')).toBeVisible({ timeout: 10_000 })
      await expect(page.getByRole('button', { name: 'Quit' })).toHaveCount(0)
    })

    test('no update found: it says so and offers the website and Quit', async ({ page }) => {
      await page.goto('/?startup=newer')
      await page.getByRole('button', { name: 'Update now' }).click()
      await expect(page.getByText('No update was found for this copy.')).toBeVisible()
      await page.getByRole('button', { name: 'Get Fuselane from fuselane.app' }).click()
      expect(await action(page)).toBe('get-fuselane')
      await page.getByRole('button', { name: 'Quit' }).click()
      expect(await action(page)).toBe('quit')
      await noSideways(page)
    })

    test('a failed check or download is shown inline, with a way out', async ({ page }) => {
      await page.goto('/?startup=newer&update=offline')
      await page.getByRole('button', { name: 'Update now' }).click()
      const alert = page.getByRole('alert')
      await expect(alert).toContainText("Couldn't check for updates: you're offline.")
      await expect(alert).toContainText('get the newest Fuselane from fuselane.app')
      await expect(
        page.getByRole('button', { name: 'Get Fuselane from fuselane.app' }),
      ).toBeVisible()
      await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible()

      await page.goto('/?startup=newer&update=bad')
      await page.getByRole('button', { name: 'Update now' }).click()
      await expect(page.getByRole('alert')).toContainText('every network dropped at 22.0 MB', {
        timeout: 10_000,
      })
      await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible()
      await expect(page.getByRole('button', { name: 'Quit' })).toBeVisible()
    })

    test('cancelling the download goes back to the start', async ({ page }) => {
      await page.goto('/?startup=newer&update=1')
      await page.getByRole('button', { name: 'Update now' }).click()
      await page.getByRole('button', { name: 'Cancel' }).click()
      await expect(page.getByRole('button', { name: 'Update now' })).toBeEnabled()
      await expect(page.getByRole('alert')).toHaveCount(0)
    })

    test('any other problem shows the message, the details and what to do', async ({ page }) => {
      await page.goto('/?startup=other')
      await expect(page.getByRole('heading', { name: "Fuselane can't start" })).toBeVisible()
      await expect(page.getByText("Fuselane isn't allowed to change its folder.")).toBeVisible()
      await expect(
        page.getByText(
          'Check that your account can write to the folder, then open Fuselane again.',
        ),
      ).toBeVisible()
      await expect(page.getByLabel('Error details')).toContainText('Permission denied')
      await noSideways(page)
      await page.getByRole('button', { name: 'Open the folder' }).click()
      expect(await action(page)).toBe('reveal')
      await page.getByRole('button', { name: 'Copy details' }).click()
      expect(await action(page)).toBe('copy')
      await expect(page.getByRole('button', { name: 'Copied' })).toBeVisible()
      await page.getByRole('button', { name: 'Quit' }).click()
      expect(await action(page)).toBe('quit')
    })
  })
}

test('the problem screens are in Hindi too', async ({ page }) => {
  await page.goto('/?startup=newer&lang=hi')
  await expect(
    page.getByRole('heading', { name: 'आपकी डाउनलोड सूची Fuselane के नए वर्ज़न की है' }),
  ).toBeVisible()
  await expect(page.getByRole('button', { name: 'अभी अपडेट करें' })).toBeVisible()
  await page.goto('/?startup=other&lang=hi')
  await expect(page.getByRole('heading', { name: 'Fuselane शुरू नहीं हो पा रहा' })).toBeVisible()
  await expect(page.getByText('Fuselane को अपना फ़ोल्डर बदलने की अनुमति नहीं है।')).toBeVisible()
  await expect(page.getByRole('button', { name: 'फ़ोल्डर खोलें' })).toBeVisible()
})

test('the problem screens pass axe in both themes', async ({ page }) => {
  for (const startup of ['newer', 'other']) {
    for (const theme of ['dark', 'light']) {
      await page.addInitScript((t) => localStorage.setItem('fuselane.theme', t), theme)
      await page.goto(`/?startup=${startup}`)
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
      const r = await new AxeBuilder({ page }).analyze()
      expect(r.violations.map((v) => `${startup}/${theme}: ${v.id}`)).toEqual([])
    }
  }
})

test('without ?startup the app starts as usual', async ({ page }) => {
  await page.goto('/?empty=1')
  await expect(page.getByRole('navigation', { name: 'Main' })).toBeVisible()
  await expect(page.locator('.startup')).toHaveCount(0)
})
