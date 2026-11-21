import { expect, test, type Page } from '@playwright/test'

// A call that fails must say so, with a way to try again, not look like "nothing here".

const heal = (page: Page) =>
  page.evaluate(() => (window as unknown as { __heal: () => void }).__heal())

/** Makes `method` fail (as the core would: a UiError) until `heal` is called. */
async function failUntilHealed(page: Page, method: string) {
  await expect(page.getByText('Demo data')).toBeVisible() // the backend is connected
  await page.evaluate(async (name) => {
    const path = '/src/lib/store.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/store')
    const b = m.useApp.getState().backend as unknown as Record<string, (...a: unknown[]) => unknown>
    const real = b[name]!.bind(b)
    let failing = true
    ;(window as unknown as { __heal: () => void }).__heal = () => (failing = false)
    b[name] = (...args: unknown[]) => {
      if (failing) {
        return Promise.reject({
          code: 'load-failed',
          message: "Fuselane couldn't read that just now.",
          hint: 'Try again in a moment.',
        })
      }
      return real(...args)
    }
  }, method)
}

test('browsers that cannot be listed say so and can be retried', async ({ page }) => {
  await page.goto('/?empty=1')
  await failUntilHealed(page, 'extensionBrowsers')
  await page.getByRole('button', { name: 'Extension' }).click()
  const alert = page.getByRole('alert').filter({ hasText: "couldn't read that just now" })
  await expect(alert).toBeVisible()
  await expect(page.getByText('No supported browser found')).toHaveCount(0)
  await heal(page)
  await alert.getByRole('button', { name: 'Try again' }).click()
  await expect(alert).toBeHidden()
  await expect(page.getByRole('list', { name: 'Your browsers' })).toBeVisible()
})

test('usage that cannot be loaded says so instead of showing an empty chart', async ({ page }) => {
  await page.goto('/?empty=1')
  await failUntilHealed(page, 'usageHistory')
  await page.getByRole('button', { name: 'Networks' }).first().click()
  await page.getByRole('radio', { name: 'Usage' }).click()
  const alert = page.getByRole('alert').filter({ hasText: "couldn't read that just now" })
  await expect(alert).toBeVisible()
  await heal(page)
  await alert.getByRole('button', { name: 'Try again' }).click()
  await expect(alert).toBeHidden({ timeout: 8000 })
})

test('a watch folder that cannot be read says so and keeps the switch off until it can', async ({
  page,
}) => {
  await page.goto('/?empty=1')
  await failUntilHealed(page, 'watchState')
  await page.getByRole('button', { name: 'Settings' }).first().click()
  const alert = page.getByRole('alert').filter({ hasText: "couldn't read that just now" })
  await expect(alert).toBeVisible()
  const toggle = page.getByRole('switch', { name: 'Add files from a folder' })
  await expect(toggle).toBeDisabled()
  await heal(page)
  await alert.getByRole('button', { name: 'Try again' }).click()
  await expect(alert).toBeHidden()
  await expect(toggle).toBeEnabled()
})
