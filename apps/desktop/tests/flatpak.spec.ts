import { expect, test, type Page } from '@playwright/test'

// The app as a Flatpak (?flatpak=1 in the demo backend; flatpak.rs in the app).

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

test('a Flatpak never offers an update: no banner, and Settings says where they come from', async ({
  page,
}) => {
  // update=1 would offer one outside a Flatpak.
  await page.goto('/?empty=1&update=1&flatpak=1')
  await page.waitForTimeout(800) // past the launch check
  await expect(page.locator('.update-banner')).toHaveCount(0)
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByTestId('updates-flatpak')).toContainText(
    'Updates come through your software centre (Flatpak).',
  )
  await expect(page.getByRole('button', { name: 'Check for updates' })).toHaveCount(0)
})

test('outside a Flatpak the update row and banner are unchanged', async ({ page }) => {
  await page.goto('/?empty=1&update=1')
  await expect(page.locator('.update-banner')).toBeVisible()
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('button', { name: /Check (for updates|again)/ })).toBeVisible()
  await expect(page.getByTestId('updates-flatpak')).toHaveCount(0)
  await expect(page.getByTestId('extension-flatpak')).toHaveCount(0)
  await expect(page.getByTestId('when-done-flatpak')).toHaveCount(0)
  await expect(page.getByRole('radio', { name: 'Sleep' })).toBeVisible()
})

test('a Flatpak says the browser extension cannot reach it, in Settings and the welcome', async ({
  page,
}) => {
  await page.goto('/?empty=1&flatpak=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const note =
    "The browser extension can't talk to the Flatpak version yet; use the .deb or AppImage for it."
  await expect(page.getByTestId('extension-flatpak')).toContainText(note)
  await page.getByRole('button', { name: 'Show it again' }).click()
  const welcome = page.getByRole('dialog')
  await expect(welcome).toBeVisible()
  // The tips are on a later step of the tour.
  for (let i = 0; i < 4 && !(await welcome.getByText(note).isVisible()); i++) {
    const next = welcome.getByRole('button', { name: /Next|Continue/ })
    if (!(await next.count())) break
    await next.first().click()
  }
  await expect(welcome.getByText(note)).toBeVisible()
  await expect(welcome.getByText('Get it from fuselane.app')).toHaveCount(0)
})

test('a Flatpak offers no Sleep or Shut down when everything finishes, and says why', async ({
  page,
}) => {
  await page.goto('/?empty=1&flatpak=1')
  await page.getByRole('button', { name: 'Settings' }).click()
  const group = page.getByRole('radiogroup', { name: 'When everything finishes' })
  await expect(group.getByRole('radio')).toHaveText(['Nothing', 'Quit'])
  await expect(page.getByTestId('when-done-flatpak')).toContainText(
    "can't put the computer to sleep or shut it down",
  )
  // Start at login and Keep awake stay: portals do them in a Flatpak.
  await expect(page.getByRole('switch', { name: 'Start at login' })).toBeVisible()
  await expect(page.getByRole('switch', { name: 'Keep the computer awake' })).toBeVisible()
  await group.getByRole('radio', { name: 'Quit' }).click()
  await expect(group.getByRole('radio', { name: 'Quit' })).toHaveAttribute('aria-checked', 'true')
})
