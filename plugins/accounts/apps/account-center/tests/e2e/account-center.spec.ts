import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

test('standalone account review is usable, secret-free, and accessible', async ({ page }, testInfo) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Accounts & access' })).toBeVisible()
  await expect(page.getByRole('row', { name: /Example Edge/ })).toBeVisible()
  await page.getByRole('button', { name: 'Review', exact: true }).click()
  await expect(page.getByRole('dialog', { name: /dns\.list access/ })).toContainText('opaque, revocable lease')
  const text = await page.locator('body').innerText()
  expect(text).not.toMatch(/access[_ -]?token|refresh[_ -]?token|api[_ -]?key|Bearer/i)
  const results = await new AxeBuilder({ page }).analyze()
  expect(results.violations).toEqual([])
  await page.screenshot({ path: `../../artifacts/account-center-${testInfo.project.name}.png`, fullPage: true })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true)
})

test('same components render in Elegy Studio embed mode', async ({ page }) => {
  await page.goto('/?embed=1')
  await expect(page.getByRole('navigation', { name: 'Account Center' })).toHaveCount(0)
  await expect(page.getByRole('row', { name: /Example Edge/ })).toBeVisible()
  if ((page.viewportSize()?.width ?? 1000) <= 620) await page.getByRole('button', { name: /Select Example Edge account/ }).click()
  await expect(page.getByRole('complementary', { name: 'Account details' })).toBeVisible()
})

test('Brave discovery handoff opens the matching provider decision', async ({ page }) => {
  await page.goto('/?connect=example-edge&discovered=brave')
  const dialog = page.getByRole('dialog', { name: 'Connect account' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('button', { name: /Continue with Example Edge/ })).toBeVisible()
  await expect(dialog).toContainText('Passwords and browser cookies are never imported')
})
