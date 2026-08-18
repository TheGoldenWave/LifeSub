import { expect, test } from '@playwright/test'

test('用户完成记录、检索、修订和设置浏览', async ({ page }) => {
  await page.goto('/')

  await page.getByRole('button', { name: '开始记录' }).click()
  await expect(page.getByText('正在记录')).toBeVisible()
  await page.getByRole('button', { name: '暂停' }).click()
  await expect(page.getByText('已暂停')).toBeVisible()
  await page.getByRole('button', { name: '继续' }).click()
  await page.getByRole('button', { name: '停止' }).click()
  await expect(page.getByText('记录已封存')).toBeVisible()

  await page.getByRole('searchbox', { name: '搜索转写' }).fill('证据链')
  await expect(page.getByText(/证据链必须保留原始转写/)).toBeVisible()
  await expect(page.getByText(/先确认首版范围/)).toBeHidden()

  await page.getByRole('button', { name: '创建修订' }).click()
  await page.getByRole('textbox', { name: '修订文本' }).fill('首版重点是可靠、可定位的声音证据。')
  await page.getByRole('button', { name: '保存修订' }).click()
  await expect(page.getByRole('button', { name: '查看原始 r1' })).toBeVisible()

  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
  await expect(page.getByText('云端处理默认关闭')).toBeVisible()
})
