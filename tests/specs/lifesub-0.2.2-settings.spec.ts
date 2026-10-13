import { expect, test } from '@playwright/test'
import path from 'path'

for (const viewport of [
  { width: 1180, height: 760 },
  { width: 760, height: 620 },
]) {
  test(`0.2.2 settings stay usable at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()

    const dialog = page.getByRole('dialog', { name: '设置' })
    await expect(dialog.getByRole('tab')).toHaveCount(3)
    await expect(dialog.getByRole('tab', { name: '录音' })).toHaveAttribute('aria-selected', 'true')
    await dialog.getByRole('tab', { name: 'ASR 与模型' }).click()
    await expect(dialog.getByRole('heading', { name: 'ASR 设置' })).toBeVisible()
    await expect(dialog.getByRole('heading', { name: '模型' })).toBeVisible()
    await dialog.getByRole('tab', { name: '诊断' }).click()
    await expect(dialog.getByRole('heading', { name: '诊断' })).toBeVisible()

    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
    expect(overflow).toBeLessThanOrEqual(0)
    await page.screenshot({
      path: path.resolve('output', 'playwright', `lifesub-0.2.2-settings-${viewport.width}.png`),
    })
  })
}
