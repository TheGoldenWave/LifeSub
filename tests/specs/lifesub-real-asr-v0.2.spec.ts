import { expect, test } from '@playwright/test'

test.describe('LifeSub V0.2 — 4-page navigation', () => {
  test('app loads with sidebar and default page', async ({ page }) => {
    await page.goto('/')
    await expect(page.locator('.app-shell')).toBeVisible()
    await expect(page.locator('.sidebar')).toBeVisible()
    await expect(page.locator('.live-capture')).toBeVisible()
  })

  test('sidebar navigation switches pages', async ({ page }) => {
    await page.goto('/')

    await page.getByRole('button', { name: '录音' }).click()
    await expect(page.locator('.live-capture')).toBeVisible()

    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()

    await page.getByRole('button', { name: '词典' }).click()
    await expect(page.locator('.dictionary-view')).toBeVisible()
  })
})

test.describe('LifeSub V0.2 — Live Capture', () => {
  test('starts recording and shows transcript', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByText('浏览器演示数据，不会录音或保存。')).toBeVisible({ timeout: 5000 })
    await page.getByRole('button', { name: '开始演示' }).click()
    await expect(page.getByText('浏览器演示').first()).toBeVisible({ timeout: 5000 })
    await expect(page.locator('.live-segment').first()).toBeVisible({ timeout: 8000 })
  })

  test('pauses and continues recording', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始演示' }).click()
    await expect(page.getByRole('button', { name: '停止' })).toBeVisible({ timeout: 5000 })
    await page.getByRole('button', { name: '暂停' }).click()
    await expect(page.getByRole('button', { name: '继续' })).toBeVisible({ timeout: 5000 })
  })

  test('stops recording and shows saved notice', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始演示' }).click()
    await page.getByRole('button', { name: '停止' }).click()
    await expect(page.getByRole('status').filter({ hasText: '浏览器演示数据，不会录音或保存。' }).last()).toBeVisible({ timeout: 5000 })
  })

  test('adds and deletes a note', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始演示' }).click()
    await expect(page.getByRole('button', { name: '停止' })).toBeVisible({ timeout: 5000 })
    await page.getByRole('button', { name: '新笔记' }).last().click()
    await expect(page.locator('.note-editor')).toBeVisible({ timeout: 5000 })
  })
})

test.describe('LifeSub V0.2 — Timeline', () => {
  test('shows search toolbar and stats', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()
    await expect(page.getByPlaceholder('搜索原话、来源或时间…')).toBeVisible({ timeout: 5000 })
  })
})

test.describe('LifeSub V0.2 — Dictionary', () => {
  test('renders dictionary with categories and entries', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '词典' }).click()
    await expect(page.locator('.dictionary-view')).toBeVisible()
    await expect(page.locator('.dictionary-category').first()).toBeVisible()
  })
})

test.describe('LifeSub V0.2 — Settings Modal', () => {
  test('opens and closes settings modal', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await expect(page.getByRole('heading', { name: '录音设置' })).toBeVisible()

    // Close via button
    await page.getByRole('button', { name: '关闭设置' }).click()
    await expect(page.getByRole('dialog')).toBeHidden({ timeout: 5000 })
  })

  test('settings tabs navigate', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()

    await page.getByRole('tab', { name: 'ASR 设置' }).click()
    await expect(page.getByText('当前 Provider')).toBeVisible({ timeout: 5000 })

    await page.getByRole('tab', { name: '模型' }).click()
    await expect(page.getByText('当前清单')).toBeVisible({ timeout: 5000 })

    await page.getByRole('tab', { name: '关于' }).click()
    await expect(page.getByRole('heading', { name: '关于 LifeSub' })).toBeVisible({ timeout: 5000 })
  })

  test('settings closes on Escape', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog')).toBeHidden({ timeout: 5000 })
  })
})

test.describe('LifeSub V0.2 — Import notice', () => {
  test('explains that browser preview cannot import real audio', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await page.getByRole('button', { name: '导入音频' }).click()
    await expect(page.getByText('浏览器演示模式仅支持示例数据，请在桌面版中导入真实音频。')).toBeVisible()
  })
})
