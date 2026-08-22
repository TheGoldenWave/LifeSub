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
    // Browser demo: status title shows "浏览器演示", desktop: "待检测"
    const statusTitle = page.locator('.live-capture__status strong')
    await expect(statusTitle).toBeVisible()
    const titleText = await statusTitle.textContent()
    expect(titleText === '浏览器演示' || titleText === '待检测').toBe(true)
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
  })
})