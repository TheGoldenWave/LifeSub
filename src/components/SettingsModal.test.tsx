import { useState } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const adapterMocks = vi.hoisted(() => ({
  loadVoiceprints: vi.fn(),
  loadAsrConfig: vi.fn(),
  saveAsrConfig: vi.fn(),
  renameVoiceprintAdapter: vi.fn(),
  deleteVoiceprintAdapter: vi.fn(),
  loadRecordingConfig: vi.fn(),
  saveRecordingConfig: vi.fn(),
  loadSettingsCapabilities: vi.fn(),
  loadModelCatalog: vi.fn(),
  installAsrModelAdapter: vi.fn(),
  cancelModelDownloadAdapter: vi.fn(),
  retryModelDownloadAdapter: vi.fn(),
  loadDiagnostics: vi.fn(),
  exportDiagnostics: vi.fn(),
  openMacosPrivacySettings: vi.fn(),
  loadRuntimeInfo: vi.fn(),
}))

const microphoneMocks = vi.hoisted(() => ({
  refresh: vi.fn(),
  select: vi.fn(),
  synchronizeConfig: vi.fn(),
  setLocked: vi.fn(),
  state: {
    phase: 'ready' as const,
    availability: 'ready' as const,
    devices: [{
      uid: 'mic-built-in',
      name: 'MacBook Pro 麦克风',
      isDefault: true,
      isConnected: true,
      transport: 'built_in' as const,
    }],
    selectedUid: 'mic-built-in' as string | null,
    selectedName: 'MacBook Pro 麦克风' as string | null,
    captureMode: 'smart' as const,
    error: null as string | null,
    refreshing: false,
    saving: false,
    locked: false,
  },
}))

vi.mock('../data/adapter', () => ({
  loadVoiceprints: adapterMocks.loadVoiceprints,
  loadAsrConfig: adapterMocks.loadAsrConfig,
  saveAsrConfig: adapterMocks.saveAsrConfig,
  renameVoiceprintAdapter: adapterMocks.renameVoiceprintAdapter,
  deleteVoiceprintAdapter: adapterMocks.deleteVoiceprintAdapter,
  loadRecordingConfig: adapterMocks.loadRecordingConfig,
  saveRecordingConfig: adapterMocks.saveRecordingConfig,
  loadSettingsCapabilities: adapterMocks.loadSettingsCapabilities,
  loadModelCatalog: adapterMocks.loadModelCatalog,
  installAsrModelAdapter: adapterMocks.installAsrModelAdapter,
  cancelModelDownloadAdapter: adapterMocks.cancelModelDownloadAdapter,
  retryModelDownloadAdapter: adapterMocks.retryModelDownloadAdapter,
  loadDiagnostics: adapterMocks.loadDiagnostics,
  exportDiagnostics: adapterMocks.exportDiagnostics,
  openMacosPrivacySettings: adapterMocks.openMacosPrivacySettings,
  loadRuntimeInfo: adapterMocks.loadRuntimeInfo,
}))

vi.mock('../microphoneDevices', () => ({
  useMicrophoneDevices: () => ({
    ...microphoneMocks.state,
    refresh: microphoneMocks.refresh,
    select: microphoneMocks.select,
    synchronizeConfig: microphoneMocks.synchronizeConfig,
    setLocked: microphoneMocks.setLocked,
  }),
}))

import { Modal } from './Modal'
import { SettingsModal } from './SettingsModal'
import { AsrSettings } from './AsrSettings'
import { RecordingSettings } from './RecordingSettings'

beforeEach(() => {
  Object.values(adapterMocks).forEach((mock) => mock.mockReset())
  microphoneMocks.refresh.mockReset()
  microphoneMocks.select.mockReset()
  microphoneMocks.synchronizeConfig.mockReset()
  microphoneMocks.setLocked.mockReset()
  Object.assign(microphoneMocks.state, {
    phase: 'ready',
    availability: 'ready',
    devices: [{
      uid: 'mic-built-in',
      name: 'MacBook Pro 麦克风',
      isDefault: true,
      isConnected: true,
      transport: 'built_in',
    }],
    selectedUid: 'mic-built-in',
    selectedName: 'MacBook Pro 麦克风',
    captureMode: 'smart',
    error: null,
    refreshing: false,
    saving: false,
    locked: false,
  })
})

function ModalHarness() {
  const [open, setOpen] = useState(false)

  return (
    <div data-testid="app-root">
      <button type="button" onClick={() => setOpen(true)}>
        打开设置
      </button>
      <div data-testid="background-copy">背景内容</div>
      <Modal open={open} onClose={() => setOpen(false)} title="测试设置">
        <button type="button">第一个字段</button>
        <button type="button">第二个字段</button>
      </Modal>
    </div>
  )
}

function HiddenFocusableModalHarness() {
  return (
    <Modal open onClose={() => undefined} title="隐藏控件测试">
      <button type="button">可见字段</button>
      <div hidden><button type="button">hidden 字段</button></div>
      <div aria-hidden="true"><button type="button">aria-hidden 字段</button></div>
      <div inert><button type="button">inert 字段</button></div>
    </Modal>
  )
}

function NestedModalHarness() {
  const [outerOpen, setOuterOpen] = useState(false)
  const [innerOpen, setInnerOpen] = useState(false)
  const [clicked, setClicked] = useState(false)

  return (
    <div data-testid="app-root">
      <button type="button" onClick={() => setOuterOpen(true)}>
        打开外层
      </button>
      <Modal open={outerOpen} onClose={() => setOuterOpen(false)} title="外层设置">
        <button type="button" onClick={() => setInnerOpen(true)}>
          打开内层
        </button>
        <button type="button">外层字段</button>
        <Modal open={innerOpen} onClose={() => setInnerOpen(false)} title="内层设置">
          <button type="button" onClick={() => setClicked(true)}>内层字段</button>
        </Modal>
      </Modal>
      {clicked && <p>内层已点击</p>}
    </div>
  )
}

function SiblingModalHarness() {
  const [leftOpen, setLeftOpen] = useState(false)
  const [rightOpen, setRightOpen] = useState(false)

  return (
    <div data-testid="app-root">
      <button type="button" onClick={() => {
        setLeftOpen(true)
        setRightOpen(true)
      }}>
        打开并列弹窗
      </button>
      <div data-testid="shared-background">共享背景</div>
      <Modal open={leftOpen} onClose={() => setLeftOpen(false)} title="左侧设置">
        <button type="button">关闭左侧内容</button>
      </Modal>
      <Modal open={rightOpen} onClose={() => setRightOpen(false)} title="右侧设置">
        <button type="button">关闭右侧内容</button>
        <button type="button" onClick={() => setLeftOpen(false)}>
          从顶层关闭左侧
        </button>
      </Modal>
    </div>
  )
}

describe('Modal accessibility', () => {
  it('excludes controls inside hidden, inert, or aria-hidden ancestors from the focus trap', async () => {
    const user = userEvent.setup()
    render(<HiddenFocusableModalHarness />)

    const closeButton = await screen.findByRole('button', { name: '关闭设置' })
    await waitFor(() => expect(closeButton).toHaveFocus())
    await user.tab({ shift: true })
    expect(screen.getByRole('button', { name: '可见字段' })).toHaveFocus()
  })

  it('moves focus into the dialog, traps Tab navigation, marks the background inert, and restores focus on close', async () => {
    const user = userEvent.setup()
    render(<ModalHarness />)

    const trigger = screen.getByRole('button', { name: '打开设置' })
    await user.click(trigger)

    const closeButton = screen.getByRole('button', { name: '关闭设置' })
    const firstField = screen.getByRole('button', { name: '第一个字段' })
    const secondField = screen.getByRole('button', { name: '第二个字段' })

    await waitFor(() => expect(closeButton).toHaveFocus())

    const appRoot = screen.getByTestId('app-root').parentElement as HTMLElement
    expect(appRoot).toHaveAttribute('aria-hidden', 'true')
    expect(appRoot).toHaveAttribute('inert')

    await user.tab()
    expect(firstField).toHaveFocus()

    await user.tab()
    expect(secondField).toHaveFocus()

    await user.tab()
    expect(closeButton).toHaveFocus()

    await user.tab({ shift: true })
    expect(secondField).toHaveFocus()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(trigger).toHaveFocus()
    expect(appRoot).not.toHaveAttribute('aria-hidden')
    expect(appRoot).not.toHaveAttribute('inert')
  })

  it('only lets the top-most modal handle Escape and keeps the lower modal inert until the stack unwinds', async () => {
    const user = userEvent.setup()
    render(<NestedModalHarness />)

    await user.click(screen.getByRole('button', { name: '打开外层' }))
    await user.click(screen.getByRole('button', { name: '打开内层' }))

    expect(screen.getAllByRole('dialog', { hidden: true })).toHaveLength(2)

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '内层设置' })).not.toBeInTheDocument())
    expect(screen.getByRole('dialog', { name: '外层设置', hidden: true })).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '外层设置' })).not.toBeInTheDocument())
  })

  it('lets the nested inner modal receive focus and clicks after being portaled to the shared host', async () => {
    const user = userEvent.setup()
    render(<NestedModalHarness />)

    await user.click(screen.getByRole('button', { name: '打开外层' }))
    await user.click(screen.getByRole('button', { name: '打开内层' }))

    const innerButton = screen.getByRole('button', { name: '内层字段' })
    innerButton.focus()
    expect(innerButton).toHaveFocus()

    await user.click(innerButton)
    expect(screen.getByText('内层已点击')).toBeInTheDocument()
  })

  it('keeps sibling modals interactive and preserves the original background state when closing in top-first order', async () => {
    const user = userEvent.setup()
    render(<SiblingModalHarness />)

    const appRoot = screen.getByTestId('app-root').parentElement as HTMLElement
    const background = screen.getByTestId('shared-background')
    background.setAttribute('aria-hidden', 'false')

    await user.click(screen.getByRole('button', { name: '打开并列弹窗' }))

    expect(appRoot).toHaveAttribute('inert')
    expect(screen.getAllByRole('dialog', { hidden: true })).toHaveLength(2)
    const dialogs = screen.getAllByRole('dialog', { hidden: true })
    expect(dialogs[0].closest('[data-modal-root="true"]')).toHaveAttribute('inert')
    expect(dialogs[0].closest('[data-modal-root="true"]')).toHaveAttribute('aria-hidden', 'true')
    expect(dialogs[1].closest('[data-modal-root="true"]')).not.toHaveAttribute('inert')

    await user.keyboard('{Escape}')
    expect(appRoot).toHaveAttribute('inert')
    expect(screen.getByRole('dialog', { name: '左侧设置', hidden: true })).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(appRoot).not.toHaveAttribute('inert')
    expect(background).toHaveAttribute('aria-hidden', 'false')
  })

  it('keeps the background locked when a lower sibling modal closes before the top-most one', async () => {
    const user = userEvent.setup()
    render(<SiblingModalHarness />)

    const appRoot = screen.getByTestId('app-root').parentElement as HTMLElement
    await user.click(screen.getByRole('button', { name: '打开并列弹窗' }))
    const dialogs = screen.getAllByRole('dialog', { hidden: true })
    expect(dialogs[0].closest('[data-modal-root="true"]')).toHaveAttribute('inert')

    await user.click(screen.getByRole('button', { name: '从顶层关闭左侧' }))
    expect(appRoot).toHaveAttribute('inert')
    expect(screen.getByRole('dialog', { name: '右侧设置', hidden: true })).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(appRoot).not.toHaveAttribute('inert')
  })

  it('removes the shared portal host after the last modal closes', async () => {
    const user = userEvent.setup()
    render(<ModalHarness />)

    await user.click(screen.getByRole('button', { name: '打开设置' }))
    expect(document.getElementById('lifesub-modal-host')).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(document.getElementById('lifesub-modal-host')).not.toBeInTheDocument())
  })
})

describe('Settings modal runtime data', () => {
  beforeEach(() => {
    adapterMocks.loadRecordingConfig.mockResolvedValue({
      capture_mode: 'smart',
      im_detection_enabled: true,
      im_apps: ['wechat'],
      detection_delay_secs: 3,
      recovery_delay_secs: 5,
      sample_rate: 48000,
      storage_path: '/tmp/legacy-recordings',
      microphone_device_uid: null,
      microphone_device_name: null,
    })
    adapterMocks.saveRecordingConfig.mockResolvedValue(undefined)
    adapterMocks.loadSettingsCapabilities.mockResolvedValue({
      production_capture_output_sample_rate_hz: 16000,
      data_directory_path: '/Users/test/Library/Application Support/LifeSub',
      im_detection_available: false,
      supported_capture_modes: ['smart', 'mic_only', 'system_only'],
    })
    adapterMocks.loadAsrConfig.mockResolvedValue({
      provider: 'sense_voice',
      model_id: 'sense-voice-small-int8-2024-07-17',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: true,
      whisper_task: null,
    })
    adapterMocks.saveAsrConfig.mockResolvedValue(undefined)
    adapterMocks.loadVoiceprints.mockResolvedValue([])
    adapterMocks.renameVoiceprintAdapter.mockResolvedValue(undefined)
    adapterMocks.deleteVoiceprintAdapter.mockResolvedValue(undefined)
    adapterMocks.loadModelCatalog.mockResolvedValue([
      {
        model_id: 'whisper-base-test',
        display_name: 'Whisper Base Test',
        provider: 'whisper',
        manifest_version: '9',
        bundle_identity: 'bundle-1',
        supported_languages: ['auto', 'en'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
      {
        model_id: 'sense-voice-small-int8-2024-07-17',
        display_name: 'SenseVoice Small',
        provider: 'sense_voice',
        manifest_version: '1',
        bundle_identity: 'bundle-2',
        supported_languages: ['auto', 'zh', 'en', 'ja', 'ko', 'yue'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
      {
        model_id: 'qwen3-asr-0.6b-int8-2026-03-25',
        display_name: 'Qwen 0.6B',
        provider: 'qwen3_asr',
        manifest_version: '1',
        bundle_identity: 'bundle-3',
        supported_languages: ['auto'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
    ])
    adapterMocks.installAsrModelAdapter.mockResolvedValue(undefined)
    adapterMocks.loadRuntimeInfo.mockResolvedValue({
      app_version: '0.2.7-test',
      tauri_version: '2.8.4',
      frontend_stack: 'React 19 + TypeScript',
      asr_runtime: 'sherpa-onnx 1.13.5',
    })
    adapterMocks.loadDiagnostics.mockResolvedValue(null)
  })

  it('groups recording, ASR and models, and diagnostics into three setup destinations', async () => {
    const user = userEvent.setup()
    render(<SettingsModal open onClose={() => undefined} />)

    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['录音', 'ASR 与模型', '诊断'])
    await user.click(screen.getByRole('tab', { name: 'ASR 与模型' }))
    expect(await screen.findByRole('heading', { name: '模型' })).toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: '当前模型' })).toBeInTheDocument()
    await user.click(screen.getByRole('tab', { name: '诊断' }))
    expect(await screen.findByText('0.2.7-test')).toBeInTheDocument()
  })

  it('renders model and about tabs from runtime data instead of hardcoded placeholders', async () => {
    const user = userEvent.setup()
    render(<SettingsModal open onClose={() => undefined} />)

    await user.click(screen.getByRole('tab', { name: 'ASR 与模型' }))
    expect(await screen.findByText('Whisper Base Test')).toBeInTheDocument()
    expect(screen.getAllByText(/sherpa-onnx 1\.13\.5/i).length).toBeGreaterThan(0)
    expect(screen.getAllByText('已就绪').length).toBeGreaterThan(0)

    await user.click(screen.getByRole('tab', { name: '诊断' }))
    expect(await screen.findByText('0.2.7-test')).toBeInTheDocument()
    expect(screen.getByText('2.8.4')).toBeInTheDocument()
    expect(screen.getByText('sherpa-onnx 1.13.5')).toBeInTheDocument()
  })

  it('installs an installable model and reloads its qualified state', async () => {
    const user = userEvent.setup()
    const notInstalled = {
      model_id: 'sense-voice-small-int8-2024-07-17',
      display_name: 'SenseVoice Small',
      provider: 'sense_voice',
      manifest_version: '1',
      bundle_identity: 'sense-bundle',
      supported_languages: ['auto', 'zh'],
      qualification_policy: 'structural_with_pinned_runtime',
      runtime_family: 'sherpa_onnx',
      runtime_version: '1.13.5',
      artifact_count: 1,
      total_bytes: 240500355,
      license_spdx: 'LicenseRef-FunASR-Model-1.1',
      installation_state: 'not_installed',
      selectable: false,
      installable: true,
      executable: false,
      reason_code: 'model_not_installed',
      last_error_code: null,
      download: null,
    }
    adapterMocks.loadModelCatalog
      .mockResolvedValueOnce([notInstalled])
      .mockResolvedValueOnce([notInstalled])
      .mockResolvedValueOnce([{
        ...notInstalled,
        installation_state: 'runtime_qualified',
        selectable: true,
        executable: true,
        reason_code: null,
      }])

    render(<SettingsModal open onClose={() => undefined} />)
    await user.click(screen.getByRole('tab', { name: 'ASR 与模型' }))
    await user.click(await screen.findByRole('button', { name: '安装 SenseVoice Small' }))

    await waitFor(() => {
      expect(adapterMocks.installAsrModelAdapter).toHaveBeenCalledWith(
        'sense-voice-small-int8-2024-07-17',
      )
    })
    expect(await screen.findByText('已就绪')).toBeInTheDocument()
  })

  it('supports arrow-key roving between tabs', async () => {
    const user = userEvent.setup()
    render(<SettingsModal open onClose={() => undefined} />)

    const recordingTab = screen.getByRole('tab', { name: '录音' })
    recordingTab.focus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('tab', { name: 'ASR 与模型' })).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(recordingTab).toHaveFocus()
  })

  it('keeps dirty drafts mounted across tabs and asks before closing', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    render(<SettingsModal open onClose={onClose} />)

    const recordingSave = await screen.findByRole('button', { name: '保存设置' })
    expect(recordingSave).toBeDisabled()
    await user.selectOptions(screen.getByRole('combobox', { name: '默认模式' }), 'mic_only')
    expect(recordingSave).toBeEnabled()
    expect(screen.getByText('有未保存的更改')).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /录音.*未保存/ })).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: 'ASR 与模型' }))
    await user.click(screen.getByRole('tab', { name: /录音.*未保存/ }))
    expect(screen.getByRole('combobox', { name: '默认模式' })).toHaveValue('mic_only')

    await user.keyboard('{Escape}')
    expect(onClose).not.toHaveBeenCalled()
    expect(screen.getByRole('alertdialog', { name: '放弃未保存的更改？' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '关闭设置' }))
    await user.click(screen.getByRole('button', { name: '放弃更改' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('shows retry affordances when model/about runtime loads fail', async () => {
    const user = userEvent.setup()
    adapterMocks.loadModelCatalog
      .mockRejectedValueOnce(new Error('model load failed'))
      .mockRejectedValueOnce(new Error('model load failed'))
    adapterMocks.loadRuntimeInfo.mockRejectedValueOnce(new Error('runtime load failed'))

    render(<SettingsModal open onClose={() => undefined} />)

    await user.click(screen.getByRole('tab', { name: 'ASR 与模型' }))
    expect((await screen.findAllByText('model load failed')).length).toBeGreaterThan(0)
    adapterMocks.loadModelCatalog.mockResolvedValueOnce([])
    await user.click(screen.getByRole('button', { name: '重试加载模型' }))
    await waitFor(() => expect(adapterMocks.loadModelCatalog).toHaveBeenCalledTimes(3))

    await user.click(screen.getByRole('tab', { name: '诊断' }))
    expect(await screen.findByRole('status')).toHaveTextContent('runtime load failed')
    adapterMocks.loadRuntimeInfo.mockResolvedValueOnce({
      app_version: '0.2.8-test',
      tauri_version: '2.8.4',
      frontend_stack: 'React 19 + TypeScript',
      asr_runtime: 'sherpa-onnx 1.13.5',
    })
    await user.click(screen.getByRole('button', { name: '重试加载运行时信息' }))
    await waitFor(() => expect(adapterMocks.loadRuntimeInfo).toHaveBeenCalledTimes(2))
  })
})

describe('Asr settings voiceprint actions', () => {
  beforeEach(() => {
    adapterMocks.loadAsrConfig.mockResolvedValue({
      provider: 'sense_voice',
      model_id: 'sense-voice-small-int8-2024-07-17',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: true,
      whisper_task: null,
    })
    adapterMocks.saveAsrConfig.mockResolvedValue(undefined)
    adapterMocks.loadVoiceprints.mockResolvedValue([
      {
        id: 'vp-1',
        name: '张伟',
        embedding_path: '/tmp/vp-1.bin',
        dictionary_entry_id: null,
        sample_count: 3,
        updated_at: '2026-08-18T16:00:00Z',
      },
    ])
  })

  it('renames and deletes voiceprints through page-level actions, and disables incomplete registration', async () => {
    const user = userEvent.setup()

    render(<AsrSettings />)

    expect(await screen.findByText('张伟')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重命名' }))
    const nameInput = screen.getByRole('textbox', { name: '声纹名称' })
    await user.clear(nameInput)
    await user.type(nameInput, '张总')
    await user.click(screen.getByRole('button', { name: '保存重命名' }))

    await waitFor(() => {
      expect(adapterMocks.renameVoiceprintAdapter).toHaveBeenCalledWith('vp-1', '张总')
    })
    expect(await screen.findByText('张总')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '删除' }))
    expect(screen.getByText('确认删除“张总”？')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '确认删除' }))
    await waitFor(() => {
      expect(adapterMocks.deleteVoiceprintAdapter).toHaveBeenCalledWith('vp-1')
    })
    await waitFor(() => {
      expect(screen.queryByText('张总')).not.toBeInTheDocument()
    })

    const registerButton = screen.getByRole('button', { name: /注册新声纹/i })
    expect(registerButton).toBeDisabled()
    expect(registerButton).toHaveTextContent('计划中')
  })

  it('disables voiceprint actions while a mutation is pending and surfaces mutation errors', async () => {
    const user = userEvent.setup()
    let releaseRename: () => void = () => {}
    adapterMocks.renameVoiceprintAdapter.mockImplementationOnce(() => new Promise<void>((resolve) => {
      releaseRename = () => resolve()
    }))
    adapterMocks.deleteVoiceprintAdapter.mockRejectedValueOnce(new Error('delete failed'))

    render(<AsrSettings />)
    expect(await screen.findByText('张伟')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '重命名' }))
    const nameInput = screen.getByRole('textbox', { name: '声纹名称' })
    await user.clear(nameInput)
    await user.type(nameInput, '张总')
    await user.click(screen.getByRole('button', { name: '保存重命名' }))
    expect(screen.getByRole('button', { name: '重命名中…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消重命名' })).toBeDisabled()
    releaseRename()
    await screen.findByText('张总')

    await user.click(screen.getByRole('button', { name: '删除' }))
    await user.click(screen.getByRole('button', { name: '确认删除' }))
    expect(await screen.findByRole('status')).toHaveTextContent('delete failed')
  })

  it('shows save errors instead of failing silently', async () => {
    const user = userEvent.setup()
    adapterMocks.saveAsrConfig.mockRejectedValueOnce(new Error('save failed'))

    render(<AsrSettings />)
    await screen.findByText('ASR 设置')
    await user.click(screen.getByRole('checkbox', { name: '自动转写' }))
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(await screen.findByText('save failed')).toBeInTheDocument()
  })

  it('lets the user choose an exact model_id and persists it with the provider', async () => {
    const user = userEvent.setup()
    adapterMocks.loadAsrConfig.mockResolvedValueOnce({
      provider: 'sense_voice',
      model_id: 'sense-voice-small-int8-2024-07-17',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: true,
      whisper_task: null,
    })
    adapterMocks.loadModelCatalog.mockResolvedValueOnce([
      {
        model_id: 'sense-voice-small-int8-2024-07-17',
        display_name: 'SenseVoice Small',
        provider: 'sense_voice',
        manifest_version: '1',
        bundle_identity: 'sense-bundle',
        supported_languages: ['auto', 'zh'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 1,
        total_bytes: 240500355,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
      {
        model_id: 'whisper-base',
        display_name: 'Whisper Base',
        provider: 'whisper',
        manifest_version: '1',
        bundle_identity: 'whisper-bundle',
        supported_languages: ['auto', 'en'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
    ])

    render(<AsrSettings />)
    await screen.findByText('ASR 设置')

    await user.selectOptions(screen.getByRole('combobox', { name: '当前 Provider' }), 'whisper')
    await user.selectOptions(screen.getByRole('combobox', { name: '当前模型' }), 'whisper-base')
    const saveButton = screen.getByRole('button', { name: '保存设置' })
    await waitFor(() => expect(saveButton).toBeEnabled())
    await user.click(saveButton)

    expect(adapterMocks.saveAsrConfig).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'whisper',
      model_id: 'whisper-base',
    }))
  })

  it('requires an explicit model choice after changing provider', async () => {
    const user = userEvent.setup()
    adapterMocks.loadAsrConfig.mockResolvedValueOnce({
      provider: 'sense_voice',
      model_id: 'sense-voice-small-int8-2024-07-17',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: true,
    })
    adapterMocks.loadModelCatalog.mockResolvedValueOnce([
      ...['whisper-tiny', 'whisper-base'].map((model_id) => ({
        model_id,
        display_name: model_id,
        provider: 'whisper',
        manifest_version: '1',
        bundle_identity: `${model_id}-bundle`,
        supported_languages: ['auto', 'en'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 1,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      })),
    ])

    render(<AsrSettings />)
    await screen.findByText('ASR 设置')
    await user.selectOptions(screen.getByRole('combobox', { name: '当前 Provider' }), 'whisper')

    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()
    expect(adapterMocks.saveAsrConfig).not.toHaveBeenCalled()

    await user.selectOptions(screen.getByRole('combobox', { name: '当前模型' }), 'whisper-base')
    const saveButton = screen.getByRole('button', { name: '保存设置' })
    await waitFor(() => expect(saveButton).toBeEnabled())
    await user.click(saveButton)
    expect(adapterMocks.saveAsrConfig).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'whisper',
      model_id: 'whisper-base',
    }))
  })

  it('shows load errors, disables save, and retries instead of silently falling back in Tauri mode', async () => {
    const user = userEvent.setup()
    adapterMocks.loadAsrConfig.mockRejectedValueOnce(new Error('asr load failed'))
    adapterMocks.loadVoiceprints.mockRejectedValueOnce(new Error('voiceprint load failed'))

    render(<AsrSettings />)

    expect(await screen.findByRole('status')).toHaveTextContent('asr load failed')
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()

    adapterMocks.loadAsrConfig.mockResolvedValueOnce({
      provider: 'sense_voice',
      model_id: 'sense-voice-small-int8-2024-07-17',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: true,
    })
    adapterMocks.loadVoiceprints.mockResolvedValueOnce([])
    await user.click(screen.getByRole('button', { name: '重试加载 ASR 设置' }))
    await waitFor(() => expect(adapterMocks.loadAsrConfig).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()
  })
})

describe('Recording settings feedback', () => {
  beforeEach(() => {
    adapterMocks.loadRecordingConfig.mockResolvedValue({
      capture_mode: 'smart',
      im_detection_enabled: true,
      im_apps: ['wechat'],
      detection_delay_secs: 3,
      recovery_delay_secs: 5,
      sample_rate: 48000,
      storage_path: '/tmp/legacy-recordings',
      microphone_device_uid: null,
      microphone_device_name: null,
    })
    adapterMocks.loadSettingsCapabilities.mockResolvedValue({
      production_capture_output_sample_rate_hz: 16000,
      data_directory_path: '/Users/test/Library/Application Support/LifeSub',
      im_detection_available: false,
      supported_capture_modes: ['smart', 'mic_only', 'system_only'],
    })
  })

  it('renders truthful production recording capabilities instead of editable legacy sample rate and storage fields', async () => {
    render(<RecordingSettings />)

    expect(await screen.findByText('16 kHz')).toBeInTheDocument()
    expect(screen.getByText('/Users/test/Library/Application Support/LifeSub')).toBeInTheDocument()
    expect(screen.queryByRole('combobox', { name: '采样率' })).not.toBeInTheDocument()
    expect(screen.queryByDisplayValue('/tmp/legacy-recordings')).not.toBeInTheDocument()
    expect(screen.getByText('当前版本不可用')).toBeInTheDocument()
    expect(screen.queryByText('检测应用')).not.toBeInTheDocument()
    expect(screen.queryByText('检测响应时间 (秒)')).not.toBeInTheDocument()
    expect(screen.queryByText('通话结束恢复 (秒)')).not.toBeInTheDocument()
  })

  it('shows the saved disconnected microphone without replacing it with the system default', async () => {
    Object.assign(microphoneMocks.state, {
      availability: 'disconnected',
      devices: [],
      selectedUid: 'usb-missing',
      selectedName: 'Studio USB Microphone',
    })

    render(<RecordingSettings />)

    expect(await screen.findByRole('combobox', { name: '收音麦克风' })).toHaveValue('usb-missing')
    expect(screen.getByText('Studio USB Microphone（已断开）')).toBeInTheDocument()
  })

  it('persists microphone selection immediately without marking the form dirty', async () => {
    const user = userEvent.setup()
    microphoneMocks.select.mockResolvedValue(undefined)
    adapterMocks.saveRecordingConfig.mockResolvedValue(undefined)
    Object.assign(microphoneMocks.state, {
      availability: 'unselected',
      selectedUid: null,
      selectedName: null,
    })

    render(<RecordingSettings />)
    await user.selectOptions(await screen.findByRole('combobox', { name: '收音麦克风' }), 'mic-built-in')

    expect(microphoneMocks.select).toHaveBeenCalledWith('mic-built-in')
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()
    expect(screen.queryByText('有未保存的更改')).not.toBeInTheDocument()
    expect(adapterMocks.saveRecordingConfig).not.toHaveBeenCalled()
  })

  it('preserves the latest persisted microphone when saving other recording fields', async () => {
    const user = userEvent.setup()
    adapterMocks.saveRecordingConfig.mockResolvedValue(undefined)
    adapterMocks.loadRecordingConfig
      .mockResolvedValueOnce({
        capture_mode: 'smart', im_detection_enabled: false, im_apps: [], detection_delay_secs: 3,
        recovery_delay_secs: 5, sample_rate: 16000, storage_path: '/tmp', microphone_device_uid: null,
        microphone_device_name: null,
      })
      .mockResolvedValueOnce({
        capture_mode: 'smart', im_detection_enabled: false, im_apps: [], detection_delay_secs: 3,
        recovery_delay_secs: 5, sample_rate: 16000, storage_path: '/tmp', microphone_device_uid: 'mic-external',
        microphone_device_name: 'External Microphone',
      })

    render(<RecordingSettings />)
    await user.selectOptions(await screen.findByRole('combobox', { name: '默认模式' }), 'mic_only')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(adapterMocks.saveRecordingConfig).toHaveBeenCalledWith(expect.objectContaining({
      capture_mode: 'mic_only',
      microphone_device_uid: 'mic-external',
      microphone_device_name: 'External Microphone',
    }))
  })

  it('locks the settings selector while a capture lifecycle is active', async () => {
    Object.assign(microphoneMocks.state, { locked: true })

    render(<RecordingSettings />)

    expect(await screen.findByRole('combobox', { name: '收音麦克风' })).toBeDisabled()
    expect(screen.getByText('录制中不可切换')).toBeInTheDocument()
  })

  it('shows a persisted error when saving recording settings fails', async () => {
    const user = userEvent.setup()
    adapterMocks.saveRecordingConfig.mockRejectedValueOnce(new Error('recording save failed'))

    render(<RecordingSettings />)
    await screen.findByText('录音设置')
    await user.selectOptions(screen.getByRole('combobox', { name: '默认模式' }), 'mic_only')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(await screen.findByText('recording save failed')).toBeInTheDocument()
  })

  it('shows load errors, disables save, and retries instead of falling back to defaults in Tauri mode', async () => {
    const user = userEvent.setup()
    adapterMocks.loadRecordingConfig.mockRejectedValueOnce(new Error('recording load failed'))

    render(<RecordingSettings />)

    expect(await screen.findByText('recording load failed')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()

    adapterMocks.loadRecordingConfig.mockResolvedValueOnce({
      capture_mode: 'smart',
      im_detection_enabled: true,
      im_apps: ['wechat'],
      detection_delay_secs: 3,
      recovery_delay_secs: 5,
      sample_rate: 48000,
      storage_path: '/tmp/legacy-recordings',
      microphone_device_uid: null,
      microphone_device_name: null,
    })
    await user.click(screen.getByRole('button', { name: '重试加载录音设置' }))
    await waitFor(() => expect(adapterMocks.loadRecordingConfig).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()
  })
})

describe('Asr settings capability projection', () => {
  beforeEach(() => {
    adapterMocks.loadVoiceprints.mockResolvedValue([])
    adapterMocks.saveAsrConfig.mockResolvedValue(undefined)
    adapterMocks.loadAsrConfig.mockResolvedValue({
      provider: 'whisper',
      model_id: 'whisper-base-test',
      language: 'zh',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: false,
      whisper_task: 'transcribe',
    })
    adapterMocks.loadModelCatalog.mockResolvedValue([
      {
        model_id: 'sense-voice-small-int8-2024-07-17',
        display_name: 'SenseVoice Small',
        provider: 'sense_voice',
        manifest_version: '1',
        bundle_identity: 'sense-bundle',
        supported_languages: ['auto', 'zh', 'en', 'ja', 'ko', 'yue'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 1,
        total_bytes: 240500355,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
      {
        model_id: 'whisper-base-test',
        display_name: 'Whisper Base Test',
        provider: 'whisper',
        manifest_version: '1',
        bundle_identity: 'whisper-bundle',
        supported_languages: ['auto', 'en'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
      {
        model_id: 'qwen3-asr-0.6b-int8-2026-03-25',
        display_name: 'Qwen 0.6B',
        provider: 'qwen3_asr',
        manifest_version: '1',
        bundle_identity: 'qwen-bundle',
        supported_languages: ['auto'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 878702423,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
    ])
  })

  it('derives language options from the selected model and collapses unsupported persisted languages to a supported default', async () => {
    render(<AsrSettings />)

    const language = await screen.findByRole('combobox', { name: '识别语言' })
    await waitFor(() => expect(language).toHaveValue('auto'))
    const options = within(language).getAllByRole('option').map((option) => option.textContent)

    expect(options).toEqual(['自动检测', 'English'])
    expect(screen.queryByRole('option', { name: '中文' })).not.toBeInTheDocument()
  })

  it('shows only provider-applicable options and persists the whisper task through the ASR config DTO', async () => {
    const user = userEvent.setup()
    render(<AsrSettings />)

    expect(await screen.findByRole('combobox', { name: 'Whisper 任务' })).toHaveValue('transcribe')
    expect(screen.queryByText('ITN（逆文本正则化）')).not.toBeInTheDocument()

    await user.selectOptions(screen.getByRole('combobox', { name: 'Whisper 任务' }), 'translate')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(adapterMocks.saveAsrConfig).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'whisper',
      model_id: 'whisper-base-test',
      language: 'auto',
      whisper_task: 'translate',
      itn_enabled: false,
    }))

    await user.selectOptions(screen.getByRole('combobox', { name: '当前 Provider' }), 'qwen3_asr')
    await user.selectOptions(screen.getByRole('combobox', { name: '当前模型' }), 'qwen3-asr-0.6b-int8-2026-03-25')

    expect(screen.queryByRole('combobox', { name: 'Whisper 任务' })).not.toBeInTheDocument()
    expect(screen.queryByText('ITN（逆文本正则化）')).not.toBeInTheDocument()
  })
})
