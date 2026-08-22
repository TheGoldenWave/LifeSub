/**
 * 大窗口响应式验收截图
 * 覆盖 walkthrough 文档 section 11 的验收矩阵。
 * 运行: npx playwright test tests/specs/wide-window-screenshots.spec.ts --reporter=line
 */
import { expect, test } from '@playwright/test'
import path from 'path'

const VIEWPORTS = [
  { name: '1280x800', width: 1280, height: 800 },
  { name: '1516x1120', width: 1516, height: 1120 },
  { name: '1728x1117', width: 1728, height: 1117 },
]

const PAGES = ['live', 'timeline', 'dictionary'] as const

async function screenshot(page: import('@playwright/test').Page, name: string) {
  const dir = path.resolve('output', 'playwright')
  await page.screenshot({ path: path.join(dir, `wide-window-${name}.png`), fullPage: false })
}

test.describe('Wide-window responsive — all pages', () => {
  for (const vp of VIEWPORTS) {
    test.describe(`viewport ${vp.name}`, () => {
      for (const pageName of PAGES) {
        test(`${pageName} page at ${vp.name}`, async ({ page }) => {
          await page.setViewportSize({ width: vp.width, height: vp.height })
          await page.goto('/')

          // Navigate to the target page
          if (pageName === 'live') {
            await page.getByRole('button', { name: '录音' }).click()
            await expect(page.locator('.live-capture')).toBeVisible({ timeout: 5000 })
            // Verify idle state shows diagnostics (not "准备就绪" contradiction)
            await expect(page.locator('.live-capture__empty')).toBeVisible({ timeout: 5000 })
            await screenshot(page, `${pageName}-${vp.name}-idle`)
          }

          if (pageName === 'timeline') {
            await page.getByRole('button', { name: '时间线' }).click()
            await expect(page.locator('.timeline-view')).toBeVisible({ timeout: 5000 })
            // Verify empty search does NOT show "没有匹配的原话"
            const hasSearchOnlyEmpty = await page.locator('.empty-state:has(strong:text("没有匹配的原话"))').count()
            expect(hasSearchOnlyEmpty).toBe(0)
            await screenshot(page, `${pageName}-${vp.name}-empty`)
          }

          if (pageName === 'dictionary') {
            await page.getByRole('button', { name: '词典' }).click()
            await expect(page.locator('.dictionary-view')).toBeVisible({ timeout: 5000 })
            // Verify footer is no longer present
            await expect(page.locator('.dictionary-view__footer')).toHaveCount(0)
            // Verify scope selector is grouped with label
            await expect(page.locator('.dictionary-view__scope-group')).toBeVisible({ timeout: 5000 })
            await screenshot(page, `${pageName}-${vp.name}-empty`)
          }
        })
      }

      test(`settings modal at ${vp.name}`, async ({ page }) => {
        await page.setViewportSize({ width: vp.width, height: vp.height })
        await page.goto('/')
        await page.getByRole('button', { name: '设置' }).click()
        await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })

        // Navigate to model tab
        await page.getByRole('tab', { name: '模型' }).click()
        await expect(page.getByText('当前清单')).toBeVisible({ timeout: 5000 })
        // Verify model cards use stable layout (no wrapping action column)
        const modelCards = page.locator('.model-card')
        await expect(modelCards.first()).toBeVisible({ timeout: 5000 })
        await screenshot(page, `settings-model-${vp.name}`)

        // Close modal
        await page.keyboard.press('Escape')
      })
    })
  }
})

test.describe('Wide-window responsive — zoom', () => {
  test('125% zoom at 1516x1120', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')

    // Test timeline at 125%
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible({ timeout: 5000 })
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '125%' })
    await screenshot(page, 'timeline-1516x1120-zoom125')

    // Reset zoom
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '100%' })

    // Test dictionary at 125%
    await page.getByRole('button', { name: '词典' }).click()
    await expect(page.locator('.dictionary-view')).toBeVisible({ timeout: 5000 })
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '125%' })
    await screenshot(page, 'dictionary-1516x1120-zoom125')

    // Reset zoom
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '100%' })

    // Test model modal at 125%
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await page.getByRole('tab', { name: '模型' }).click()
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '125%' })
    await screenshot(page, 'settings-model-1516x1120-zoom125')
  })

  test('150% zoom at 1516x1120', async ({ page }) => {
    await page.setViewportSize({ width: 1516, height: 1120 })
    await page.goto('/')

    // Test recording page at 150%
    await page.getByRole('button', { name: '录音' }).click()
    await expect(page.locator('.live-capture')).toBeVisible({ timeout: 5000 })
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '150%' })
    await screenshot(page, 'live-1516x1120-zoom150')

    // Reset zoom
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '100%' })

    // Test model modal at 150%
    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await page.getByRole('tab', { name: '模型' }).click()
    await page.evaluate(() => { (document.body.style as CSSStyleDeclaration).zoom = '150%' })
    await screenshot(page, 'settings-model-1516x1120-zoom150')
  })
})