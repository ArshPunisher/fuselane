import { expect, test } from '@playwright/test'

// The when-done countdown must not take focus or shout every second.

test('the when-done countdown does not take focus or shout every second', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.getByRole('button', { name: 'New download' }).first().click()
  const field = page.getByRole('textbox').first()
  await field.focus()
  await page.keyboard.press('Escape')
  await page.locator('main').focus()
  await page.evaluate(async () => {
    const path = '/src/lib/store.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/store')
    m.useApp.setState({ whenDone: { action: 'sleep', endsAt: Date.now() + 60_000 } })
  })
  const banner = page.locator('.when-done')
  await expect(banner).toBeVisible()
  await expect(banner.getByRole('button', { name: 'Cancel' })).not.toBeFocused()
  await expect(banner).not.toHaveAttribute('role', 'alert')
})
