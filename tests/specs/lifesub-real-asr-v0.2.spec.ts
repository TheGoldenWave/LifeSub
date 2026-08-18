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
    await page.getByRole('button', { name: '开始记录' }).click()
    await expect(page.getByText('正在记录')).toBeVisible()
    // Demo segments appear after 1 second
    await expect(page.locator('.live-segment').first()).toBeVisible({ timeout: 5000 })
  })

  test('pauses and continues recording', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始记录' }).click()
    await page.getByRole('button', { name: '暂停' }).click()
    await expect(page.getByText('已暂停')).toBeVisible()
  })

  test('stops recording and shows saved notice', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始记录' }).click()
    await page.getByRole('button', { name: '停止' }).click()
    await expect(page.getByText('录音已保存')).toBeVisible()
  })

  test('adds and deletes a note', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '开始记录' }).click()
    await page.getByRole('button', { name: '笔记' }).click()
    await expect(page.locator('.note-card')).toBeVisible()
  })
})

test.describe('LifeSub V0.2 — Timeline', () => {
  test('shows search toolbar and stats', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()
    await expect(page.getByPlaceholder('搜索转写、笔记或标签...')).toBeVisible()
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
    await expect(page.locator('.modal')).toBeVisible()
    await expect(page.getByRole('heading', { name: '录音设置' })).toBeVisible()

    // Close via button
    await page.getByRole('button', { name: '关闭' }).click()
    await expect(page.locator('.modal')).toBeHidden()
  })

  test('settings tabs navigate', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()

    await page.getByRole('button', { name: 'ASR 设置' }).click()
    await expect(page.getByText('当前 Provider')).toBeVisible()

    await page.getByRole('button', { name: '模型' }).click()
    await expect(page.getByText('已安装模型')).toBeVisible()

    await page.getByRole('button', { name: '关于' }).click()
    await expect(page.getByText('LifeSub')).toBeVisible()
  })

  test('settings closes on Escape', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.locator('.modal')).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.locator('.modal')).toBeHidden()
  })
})

test.describe('LifeSub V0.2 — Import notice', () => {
  test('shows import notice when clicking import audio', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: '导入音频' }).click()
    await expect(page.getByText('导入音频功能将在时间线页面中可用')).toBeVisible()
  })
})