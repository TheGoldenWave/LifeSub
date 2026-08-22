import { expect, test } from '@playwright/test'

test('用户完成记录、检索、修订和设置浏览', async ({ page }) => {
  await page.goto('/')

  // Start demo recording
  await page.getByRole('button', { name: '开始演示' }).click()
  await page.getByRole('button', { name: '停止' }).click()

  // Navigate to timeline and search
  await page.getByRole('button', { name: '时间线' }).click()
  await page.getByRole('searchbox', { name: '搜索转写' }).fill('证据链')
  await expect(page.getByText(/证据链必须保留原始转写/).first()).toBeVisible()

  // Create a revision
  await page.getByRole('button', { name: '创建修订' }).click()
  await page.getByRole('textbox', { name: '修订文本' }).fill('首版重点是可靠、可定位的声音证据。')
  await page.getByRole('button', { name: '保存修订' }).click()
  await expect(page.getByRole('button', { name: '查看原始 r1' })).toBeVisible()

  // Open settings modal
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
  await expect(page.getByRole('heading', { name: '录音设置' })).toBeVisible()
})
