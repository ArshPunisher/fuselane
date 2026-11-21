import { expect, test } from '@playwright/test'

// A failed render or an unexpected error must not blank the window or leak raw text.

test('a render that throws shows a message and a way back, not an empty window', async ({
  page,
}) => {
  // The canvas context is gone (a lost GPU context, a locked-down webview).
  await page.addInitScript(() => {
    HTMLCanvasElement.prototype.getContext = () => null
  })
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  await page.goto('/')
  const alert = page.getByRole('alert').filter({ hasText: 'Something went wrong' })
  await expect(alert).toBeVisible()
  await expect(alert.getByRole('button', { name: 'Reload' })).toBeVisible()
  await expect(page.locator('#root')).not.toBeEmpty()
})

test('an unhandled promise rejection becomes a plain toast, not silence', async ({ page }) => {
  await page.goto('/?empty=1')
  await page.evaluate(() => {
    void Promise.reject(new Error("undefined is not an object (evaluating 'x.networks.map')"))
  })
  const toast = page.locator('.toast')
  await expect(toast).toBeVisible()
  await expect(toast).toContainText('Something went wrong')
  await expect(toast).not.toContainText('undefined is not an object')
})

test('an unexpected error shows the generic message, never its raw text', async ({ page }) => {
  await page.goto('/?empty=1')
  const out = await page.evaluate(async () => {
    const path = '/src/lib/backend.ts'
    const m = (await import(/* @vite-ignore */ path)) as typeof import('../src/lib/backend')
    return [
      m.toUiError(new TypeError("undefined is not an object (evaluating 'x.y')")),
      m.toUiError('command start_over not found'),
      m.toUiError({ code: 'bad-link', message: 'Paste a link to download.', hint: 'Links start.' }),
    ]
  })
  for (const e of out.slice(0, 2)) {
    expect(e.code).toBe('unexpected')
    expect(e.message).toBe('Something went wrong.')
  }
  expect(out[2]!.message).toBe('Paste a link to download.')
})
