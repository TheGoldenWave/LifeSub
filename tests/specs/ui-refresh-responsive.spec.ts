import { expect, test, type Page } from '@playwright/test'
import path from 'path'

const MOBILE_VIEWPORT = { width: 390, height: 844 }

const TAURI_RECORDING_CONFIG = {
  capture_mode: 'mic_only',
  im_detection_enabled: true,
  im_apps: [],
  detection_delay_secs: 3,
  recovery_delay_secs: 5,
  sample_rate: 16000,
  storage_path: '~/.lifesub/recordings/',
  microphone_device_uid: 'built-in-microphone',
  microphone_device_name: 'MacBook Pro 麦克风（系统默认）',
}

async function installTauriVisualFixture(page: Page, recordingConfig = TAURI_RECORDING_CONFIG) {
  await page.addInitScript(({ recordingConfig }) => {
    let callbackId = 0
    const callbacks = new Map<number, (...args: unknown[]) => unknown>()
    const devices = [
      {
        uid: 'built-in-microphone',
        name: 'MacBook Pro 麦克风（系统默认）',
        transport: 'built_in',
        isDefault: true,
        isConnected: true,
      },
      {
        uid: 'long-device-name',
        name: '会议室 USB 音频接口与超长麦克风设备名称用于布局压力测试',
        transport: 'usb',
        isDefault: false,
        isConnected: true,
      },
    ]

    Object.assign(window, {
      __TAURI_INTERNALS__: {
        transformCallback(callback: (...args: unknown[]) => unknown) {
          callbackId += 1
          callbacks.set(callbackId, callback)
          return callbackId
        },
        unregisterCallback(id: number) {
          callbacks.delete(id)
        },
        async invoke(command: string) {
          if (command === 'get_recording_config') return recordingConfig
          if (command === 'list_microphone_devices') return devices
          if (command === 'list_timeline_records') return []
          if (command === 'plugin:event|listen') return ++callbackId
          if (command === 'plugin:event|unlisten' || command === 'set_recording_config') return undefined
          throw new Error(`Unexpected visual fixture command: ${command}`)
        },
      },
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener() {},
      },
    })
  }, { recordingConfig })
}

async function expectNoMicrophoneControlCollisions(page: Page) {
  const geometry = await page.locator('.microphone-selector--compact').evaluate((selector) => {
    const label = selector.querySelector('.microphone-selector__label')
    const select = selector.querySelector('.microphone-selector__select')
    const refresh = selector.querySelector('.microphone-selector__refresh')
    const probe = selector.querySelector('.microphone-selector__probe')
    const status = selector.querySelector('.microphone-selector__status')
    if (!label || !select || !refresh || !probe || !status) throw new Error('Missing microphone selector control')

    const range = document.createRange()
    range.selectNodeContents(label)
    const labelText = range.getBoundingClientRect()
    const boxes = [select, refresh, probe].map((element) => element.getBoundingClientRect())
    const selectorBox = selector.getBoundingClientRect()
    const statusBox = status.getBoundingClientRect()
    const overlaps = (left: DOMRect, right: DOMRect) => (
      left.left < right.right
      && left.right > right.left
      && left.top < right.bottom
      && left.bottom > right.top
    )

    return {
      labelVisible: labelText.width > 0 && labelText.height > 0,
      labelCollisions: boxes.filter((box) => overlaps(labelText, box)).length,
      controlCollisions: boxes.some((box, index) => boxes.slice(index + 1).some((other) => overlaps(box, other))),
      statusContained: statusBox.left >= selectorBox.left && statusBox.right <= selectorBox.right,
    }
  })

  expect(geometry.labelVisible).toBe(true)
  expect(geometry.labelCollisions).toBe(0)
  expect(geometry.controlCollisions).toBe(false)
  expect(geometry.statusContained).toBe(true)
}

async function expectNoHorizontalOverflow(page: Page) {
  const overflow = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }))
  expect(overflow.documentWidth).toBeLessThanOrEqual(overflow.viewportWidth)
}

async function capture(page: Page, name: string) {
  await page.screenshot({
    path: path.resolve('output', 'playwright', `ui-refresh-${name}.png`),
    fullPage: false,
  })
}

test.describe('UI refresh responsive acceptance', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MOBILE_VIEWPORT)
    await page.goto('/')
  })

  test('keeps the primary navigation compact, current, and touch accessible', async ({ page }) => {
    const currentPage = page.getByRole('button', { name: '录音' })
    await expect(currentPage).toHaveAttribute('aria-current', 'page')

    for (const name of ['录音', '时间线', '词典', '设置']) {
      const box = await page.getByRole('button', { name }).boundingBox()
      expect(box?.height ?? 0).toBeGreaterThanOrEqual(44)
    }

    await expectNoHorizontalOverflow(page)
    await capture(page, 'mobile-live')
  })

  test('keeps timeline, dictionary, and settings inside the narrow viewport', async ({ page }) => {
    await page.getByRole('button', { name: '时间线' }).click()
    await expect(page.locator('.timeline-view')).toBeVisible()
    await expect(page.locator('.timeline-view__list-pane')).toBeVisible()
    await expect(page.locator('.timeline-view__detail-pane')).toBeHidden()
    await page.locator('.session-tree__select').first().click()
    await expect(page.getByRole('button', { name: '返回会话列表' })).toBeVisible()
    await expect(page.locator('.timeline-view__list-pane')).toBeHidden()
    await page.getByRole('button', { name: '返回会话列表' }).click()
    await expect(page.locator('.timeline-view__list-pane')).toBeVisible()
    const timelineGap = await page.evaluate(() => {
      const lastSession = document.querySelector('.session-tree__group:last-child')?.getBoundingClientRect()
      const stats = document.querySelector('.stats-bar')?.getBoundingClientRect()
      if (!lastSession || !stats) throw new Error('Missing timeline geometry')
      return stats.top - lastSession.bottom
    })
    expect(timelineGap).toBeLessThanOrEqual(48)
    await expectNoHorizontalOverflow(page)
    await capture(page, 'mobile-timeline')

    await page.getByRole('button', { name: '词典' }).click()
    await expect(page.locator('.dictionary-view')).toBeVisible()
    await expect(page.locator('.dictionary-view__categories')).toBeVisible()
    await expect(page.locator('.dictionary-view__entries')).toBeHidden()
    await page.locator('.dictionary-category').first().click()
    await expect(page.getByRole('button', { name: '返回分类' })).toBeVisible()
    await page.locator('.dictionary-entry').first().click()
    await expect(page.getByRole('button', { name: '返回词条' })).toBeVisible()
    await expect(page.getByRole('button', { name: '新建分类' })).toBeHidden()
    await expect(page.getByRole('button', { name: '新建词条' })).toBeHidden()
    await expect(page.getByRole('button', { name: '删除分类' })).toBeHidden()
    await expectNoHorizontalOverflow(page)
    await capture(page, 'mobile-dictionary')

    await page.getByRole('button', { name: '设置' }).click()
    await expect(page.getByRole('dialog')).toBeVisible()
    const closeButtonBox = await page.getByRole('button', { name: '关闭设置' }).boundingBox()
    expect(closeButtonBox?.width ?? 0).toBeGreaterThanOrEqual(44)
    expect(closeButtonBox?.height ?? 0).toBeGreaterThanOrEqual(44)
    await page.getByRole('tab', { name: '模型' }).click()
    await expect(page.getByText('当前清单')).toBeVisible()
    await expectNoHorizontalOverflow(page)
    await capture(page, 'mobile-settings')
  })

  test('shows a visible keyboard focus ring', async ({ page }) => {
    await page.keyboard.press('Tab')
    const focused = page.locator(':focus-visible')
    await expect(focused).toBeVisible()
    expect(await focused.evaluate((element) => getComputedStyle(element).outlineStyle)).not.toBe('none')
  })

  test('keeps active recording controls inside the mobile viewport', async ({ page }) => {
    await page.getByRole('button', { name: '播放示例转写' }).click()
    await expect(page.getByRole('button', { name: '暂停' })).toBeVisible()
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight))

    const actions = page.locator('.live-capture__bar > .live-capture__actions')
    await expect(actions).toHaveCSS('position', 'fixed')
    await expect(actions.getByLabel('当前时长')).toBeVisible()
    await expect(actions.getByRole('button', { name: '新笔记' })).toHaveCount(0)
    const box = await actions.boundingBox()
    expect(box).not.toBeNull()
    expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(MOBILE_VIEWPORT.height)
    await expectNoHorizontalOverflow(page)
    await capture(page, 'mobile-live-recording')
  })

  test('protects unsaved settings before closing', async ({ page }) => {
    await page.getByRole('button', { name: '设置' }).click()
    const saveButton = page.getByRole('button', { name: '保存设置' }).first()
    await expect(saveButton).toBeDisabled()
    const mode = page.getByRole('combobox', { name: '默认模式' })
    const currentMode = await mode.inputValue()
    await mode.selectOption(currentMode === 'mic_only' ? 'smart' : 'mic_only')
    await expect(page.getByText('有未保存的更改')).toBeVisible()
    await page.getByRole('button', { name: '关闭设置' }).click()
    await expect(page.getByRole('alertdialog', { name: '放弃未保存的更改？' })).toBeVisible()
    await page.getByRole('button', { name: '继续编辑' }).click()
    await expect(page.getByRole('dialog', { name: '设置' })).toBeVisible()
  })
})

test.describe('native recording header geometry', () => {
  for (const viewport of [
    { width: 1180, height: 760 },
    { width: 1280, height: 800 },
    { width: 1516, height: 1120 },
    MOBILE_VIEWPORT,
  ]) {
    test(`keeps microphone controls separate at ${viewport.width}x${viewport.height}`, async ({ page }) => {
      await installTauriVisualFixture(page)
      await page.setViewportSize(viewport)
      await page.goto('/')
      await expect(page.getByRole('combobox', { name: '收音麦克风' })).toHaveValue('built-in-microphone')
      await expect(page.getByRole('button', { name: '开始记录' })).toBeEnabled()
      await expectNoMicrophoneControlCollisions(page)
      await expectNoHorizontalOverflow(page)
      await page.screenshot({
        path: path.resolve('output', 'playwright', `tauri-live-fixed-${viewport.width}x${viewport.height}.png`),
        fullPage: false,
      })
    })
  }

  for (const zoom of ['125%', '150%']) {
    test(`keeps microphone controls separate at ${zoom} zoom`, async ({ page }) => {
      await installTauriVisualFixture(page)
      await page.setViewportSize({ width: 1516, height: 1120 })
      await page.goto('/')
      await page.evaluate((value) => { document.body.style.zoom = value }, zoom)
      await expectNoMicrophoneControlCollisions(page)
      await expectNoHorizontalOverflow(page)
      await page.screenshot({
        path: path.resolve('output', 'playwright', `tauri-live-fixed-1516x1120-zoom${zoom.replace('%', '')}.png`),
        fullPage: false,
      })
    })
  }

  test('contains a long selected device name', async ({ page }) => {
    await installTauriVisualFixture(page, {
      ...TAURI_RECORDING_CONFIG,
      microphone_device_uid: 'long-device-name',
      microphone_device_name: '会议室 USB 音频接口与超长麦克风设备名称用于布局压力测试',
    })
    await page.setViewportSize({ width: 1180, height: 760 })
    await page.goto('/')
    await expect(page.getByRole('combobox', { name: '收音麦克风' })).toHaveValue('long-device-name')
    await expectNoMicrophoneControlCollisions(page)
    await expectNoHorizontalOverflow(page)
  })

  test('contains a disconnected saved device name', async ({ page }) => {
    await installTauriVisualFixture(page, {
      ...TAURI_RECORDING_CONFIG,
      microphone_device_uid: 'disconnected-device',
      microphone_device_name: '已保存但当前断开的超长会议室麦克风设备名称',
    })
    await page.setViewportSize(MOBILE_VIEWPORT)
    await page.goto('/')
    await expect(page.getByText('已保存但当前断开的超长会议室麦克风设备名称 已断开')).toBeVisible()
    await expectNoMicrophoneControlCollisions(page)
    await expectNoHorizontalOverflow(page)
  })
})
