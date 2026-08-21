import { useState } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
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
  loadModelCatalog: vi.fn(),
  loadRuntimeInfo: vi.fn(),
}))

vi.mock('../data/adapter', () => ({
  loadVoiceprints: adapterMocks.loadVoiceprints,
  loadAsrConfig: adapterMocks.loadAsrConfig,
  saveAsrConfig: adapterMocks.saveAsrConfig,
  renameVoiceprintAdapter: adapterMocks.renameVoiceprintAdapter,
  deleteVoiceprintAdapter: adapterMocks.deleteVoiceprintAdapter,
  loadRecordingConfig: adapterMocks.loadRecordingConfig,
  saveRecordingConfig: adapterMocks.saveRecordingConfig,
  loadModelCatalog: adapterMocks.loadModelCatalog,
  loadRuntimeInfo: adapterMocks.loadRuntimeInfo,
}))

import { Modal } from './Modal'
import { SettingsModal } from './SettingsModal'
import { AsrSettings } from './AsrSettings'
import { RecordingSettings } from './RecordingSettings'

beforeEach(() => {
  Object.values(adapterMocks).forEach((mock) => mock.mockReset())
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
      sample_rate: 16000,
      storage_path: '~/.lifesub/recordings/',
    })
    adapterMocks.saveRecordingConfig.mockResolvedValue(undefined)
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
    ])
    adapterMocks.loadRuntimeInfo.mockResolvedValue({
      app_version: '0.2.7-test',
      tauri_version: '2.8.4',
      frontend_stack: 'React 19 + TypeScript',
      asr_runtime: 'sherpa-onnx 1.13.5',
    })
  })

  it('renders model and about tabs from runtime data instead of hardcoded placeholders', async () => {
    const user = userEvent.setup()
    render(<SettingsModal open onClose={() => undefined} />)

    await user.click(screen.getByRole('tab', { name: '模型' }))
    expect(await screen.findByText('Whisper Base Test')).toBeInTheDocument()
    expect(screen.getByText(/sherpa-onnx 1\.13\.5/i)).toBeInTheDocument()
    expect(screen.getByText('已就绪')).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: '关于' }))
    expect(await screen.findByText('0.2.7-test')).toBeInTheDocument()
    expect(screen.getByText('2.8.4')).toBeInTheDocument()
    expect(screen.getByText('sherpa-onnx 1.13.5')).toBeInTheDocument()
  })

  it('supports arrow-key roving between tabs', async () => {
    const user = userEvent.setup()
    render(<SettingsModal open onClose={() => undefined} />)

    const recordingTab = screen.getByRole('tab', { name: '录音设置' })
    recordingTab.focus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('tab', { name: 'ASR 设置' })).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(recordingTab).toHaveFocus()
  })

  it('shows retry affordances when model/about runtime loads fail', async () => {
    const user = userEvent.setup()
    adapterMocks.loadModelCatalog.mockRejectedValueOnce(new Error('model load failed'))
    adapterMocks.loadRuntimeInfo.mockRejectedValueOnce(new Error('runtime load failed'))

    render(<SettingsModal open onClose={() => undefined} />)

    await user.click(screen.getByRole('tab', { name: '模型' }))
    expect(await screen.findByRole('status')).toHaveTextContent('model load failed')
    adapterMocks.loadModelCatalog.mockResolvedValueOnce([])
    await user.click(screen.getByRole('button', { name: '重试加载模型' }))
    await waitFor(() => expect(adapterMocks.loadModelCatalog).toHaveBeenCalledTimes(2))

    await user.click(screen.getByRole('tab', { name: '关于' }))
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

  it('renames and deletes voiceprints through persisted actions, and disables incomplete registration', async () => {
    const user = userEvent.setup()
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('张总')
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)

    render(<AsrSettings />)

    expect(await screen.findByText('张伟')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重命名' }))

    await waitFor(() => {
      expect(adapterMocks.renameVoiceprintAdapter).toHaveBeenCalledWith('vp-1', '张总')
    })
    expect(await screen.findByText('张总')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '删除' }))
    await waitFor(() => {
      expect(adapterMocks.deleteVoiceprintAdapter).toHaveBeenCalledWith('vp-1')
    })
    await waitFor(() => {
      expect(screen.queryByText('张总')).not.toBeInTheDocument()
    })

    const registerButton = screen.getByRole('button', { name: /注册新声纹/i })
    expect(registerButton).toBeDisabled()
    expect(registerButton).toHaveTextContent('计划中')

    prompt.mockRestore()
    confirm.mockRestore()
  })

  it('disables voiceprint actions while a mutation is pending and surfaces mutation errors', async () => {
    const user = userEvent.setup()
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('张总')
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)
    let releaseRename: () => void = () => {}
    adapterMocks.renameVoiceprintAdapter.mockImplementationOnce(() => new Promise<void>((resolve) => {
      releaseRename = () => resolve()
    }))
    adapterMocks.deleteVoiceprintAdapter.mockRejectedValueOnce(new Error('delete failed'))

    render(<AsrSettings />)
    expect(await screen.findByText('张伟')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '重命名' }))
    expect(screen.getByRole('button', { name: '重命名中…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '删除' })).toBeDisabled()
    releaseRename()
    await screen.findByText('张总')

    await user.click(screen.getByRole('button', { name: '删除' }))
    expect(await screen.findByRole('status')).toHaveTextContent('delete failed')

    prompt.mockRestore()
    confirm.mockRestore()
  })

  it('shows save errors instead of failing silently', async () => {
    const user = userEvent.setup()
    adapterMocks.saveAsrConfig.mockRejectedValueOnce(new Error('save failed'))

    render(<AsrSettings />)
    await screen.findByText('ASR 设置')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(await screen.findByRole('status')).toHaveTextContent('save failed')
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
    expect(screen.getByRole('button', { name: '保存设置' })).toBeEnabled()
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
      sample_rate: 16000,
      storage_path: '~/.lifesub/recordings/',
    })
  })

  it('shows a persisted error when saving recording settings fails', async () => {
    const user = userEvent.setup()
    adapterMocks.saveRecordingConfig.mockRejectedValueOnce(new Error('recording save failed'))

    render(<RecordingSettings />)
    await screen.findByText('录音设置')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    expect(await screen.findByRole('status')).toHaveTextContent('recording save failed')
  })

  it('shows load errors, disables save, and retries instead of falling back to defaults in Tauri mode', async () => {
    const user = userEvent.setup()
    adapterMocks.loadRecordingConfig.mockRejectedValueOnce(new Error('recording load failed'))

    render(<RecordingSettings />)

    expect(await screen.findByRole('status')).toHaveTextContent('recording load failed')
    expect(screen.getByRole('button', { name: '保存设置' })).toBeDisabled()

    adapterMocks.loadRecordingConfig.mockResolvedValueOnce({
      capture_mode: 'smart',
      im_detection_enabled: true,
      im_apps: ['wechat'],
      detection_delay_secs: 3,
      recovery_delay_secs: 5,
      sample_rate: 16000,
      storage_path: '~/.lifesub/recordings/',
    })
    await user.click(screen.getByRole('button', { name: '重试加载录音设置' }))
    await waitFor(() => expect(adapterMocks.loadRecordingConfig).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: '保存设置' })).toBeEnabled()
  })
})
