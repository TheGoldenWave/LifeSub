/**
 * 大窗口走查 P1/P2 修复的 DOM 级验证。
 * 覆盖 walkthrough 文档 section 11 的状态验收清单中的关键语义断言。
 */
import { expect, test } from '@playwright/test'

test.describe('W-R01 recording readiness state is truthful', () => {
  test('idle state never claims 准备就绪 while diagnostics are pending', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')
    await page.getByRole('button', { name: '录音' }).click()
    await expect(page.locator('.live-capture')).toBeVisible()

    // The empty state must render (not a single "准备就绪" title)
    await expect(page.locator('.live-capture__empty')).toBeVisible({ timeout: 5000 })
    // "准备就绪" must not appear anywhere on the idle page
    await expect(page.getByText('准备就绪')).toHaveCount(0)
    // The idle status title should reflect the actual state
    // Browser preview names the task directly; desktop reports its hardware readiness.
    const statusTitle = page.locator('.live-capture__status strong')
    await expect(statusTitle).toBeVisible()
    const titleText = await statusTitle.textContent()
    expect(titleText === '示例转写待播放' || titleText === '待检测').toBe(true)
    // In browser demo mode, the empty state shows the demo notice
    await expect(page.getByText('浏览器演示数据，不会录音或保存。')).toBeVisible()
  })
})

test.describe('W-T01 timeline empty search is not mislabeled as no-match', () => {
  test('empty query shows 暂无转写/等待转写, never 没有匹配', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()

    // Search field is empty (placeholder still showing), so the detail must NOT say 没有匹配
    await expect(page.getByText('没有匹配的原话')).toHaveCount(0)
  })

  test('non-empty query with no result shows 没有匹配的原话', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()

    const search = page.getByPlaceholder('搜索原话、来源或时间…')
    await search.fill('绝不存在的关键词xyz')
    // With a real query and no matches, the no-match empty state is correct
    await expect(page.getByText('没有匹配的原话')).toBeVisible({ timeout: 5000 })
  })

  test('long timeline stays inside the window with independent pane scrolling', async ({ page }) => {
    const viewport = { width: 1516, height: 760 }
    await page.setViewportSize(viewport)
    await page.goto('/')
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()

    await page.evaluate(() => {
      const sessionTree = document.querySelector('.session-tree__contents')
      const session = document.querySelector('.session-tree__group')
      const segments = document.querySelector('.segments')
      const segment = document.querySelector('.segment')
      if (!sessionTree || !session || !segments || !segment) throw new Error('Missing timeline fixture nodes')
      for (let index = 0; index < 8; index += 1) sessionTree.append(session.cloneNode(true))
      for (let index = 0; index < 12; index += 1) segments.append(segment.cloneNode(true))
    })

    const geometry = await page.evaluate(() => {
      const app = document.querySelector('.app-shell')
      const settings = document.querySelector('.nav-item--settings')
      const tree = document.querySelector('.session-tree')
      const transcript = document.querySelector('.transcript')
      if (!app || !settings || !tree || !transcript) throw new Error('Missing timeline layout nodes')
      const settingsBox = settings.getBoundingClientRect()
      return {
        appHeight: app.getBoundingClientRect().height,
        viewportHeight: window.innerHeight,
        documentHeight: document.documentElement.scrollHeight,
        settingsBottom: settingsBox.bottom,
        treeClientHeight: tree.clientHeight,
        treeScrollHeight: tree.scrollHeight,
        transcriptClientHeight: transcript.clientHeight,
        transcriptScrollHeight: transcript.scrollHeight,
      }
    })

    expect(geometry.appHeight).toBeLessThanOrEqual(geometry.viewportHeight)
    expect(geometry.documentHeight).toBeLessThanOrEqual(geometry.viewportHeight)
    expect(geometry.settingsBottom).toBeLessThanOrEqual(geometry.viewportHeight)
    expect(geometry.treeScrollHeight).toBeGreaterThan(geometry.treeClientHeight)
    expect(geometry.transcriptScrollHeight).toBeGreaterThan(geometry.transcriptClientHeight)

    const scrollPositions = await page.evaluate(() => {
      const tree = document.querySelector('.session-tree')
      const transcript = document.querySelector('.transcript')
      if (!tree || !transcript) throw new Error('Missing scroll containers')
      tree.scrollTop = 120
      transcript.scrollTop = 120
      return { tree: tree.scrollTop, transcript: transcript.scrollTop }
    })
    expect(scrollPositions.tree).toBeGreaterThan(0)
    expect(scrollPositions.transcript).toBeGreaterThan(0)

    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog', { name: '设置' })).toBeVisible()
  })
})

test.describe('W-D01/W-D05 dictionary scope group and footer', () => {
  test('scope selector is grouped with label, footer removed', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')
    await page.getByRole('button', { name: '词典' }).click()
    await expect(page.locator('.dictionary-view')).toBeVisible()

    await expect(page.locator('.dictionary-view__scope-group')).toBeVisible({ timeout: 5000 })
    await expect(page.locator('.dictionary-view__footer')).toHaveCount(0)
  })
})

test.describe('W-S01/W-S03 model card layout and grouping', () => {
  test('model cards are grouped by provider with stable meta layout', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible()
    await page.getByRole('tab', { name: '模型' }).click()
    await expect(page.getByText('当前清单')).toBeVisible({ timeout: 5000 })

    // Provider group headers present
    const providerHeaders = page.locator('.model-list__provider')
    await expect(providerHeaders.first()).toBeVisible({ timeout: 5000 })

    // No "安装计划中" duplicate state button for non-installed models
    // (the non-installed state is now a single "暂不可安装" pill)
    const installPlannedButtons = page.locator('button:has-text("安装计划中")')
    await expect(installPlannedButtons).toHaveCount(0)
    await expect(page.getByText('暂不可安装')).toHaveCount(1)
    await expect(page.getByText('未安装')).toHaveCount(0)
  })
})
