import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  load: vi.fn(),
  exportSnapshot: vi.fn(),
  openPrivacy: vi.fn(),
  probe: vi.fn(),
  copy: vi.fn(),
}))

vi.mock('../data/adapter', () => ({
  loadDiagnostics: mocks.load,
  exportDiagnostics: mocks.exportSnapshot,
  openMacosPrivacySettings: mocks.openPrivacy,
  probeSelectedMicrophone: mocks.probe,
}))

import { DiagnosticsSettings } from './DiagnosticsSettings'

const snapshot = {
  schema: 'lifesub.diagnostics.v1',
  generated_at: '2026-08-27T00:00:00Z',
  permissions: { microphone: 'granted', screen_recording: 'not_granted' },
  selected_microphone: {
    uid: 'mic-1', name: 'Built-in Microphone', connected: true, transport: 'built_in', probe_result: 'compatible',
    probe: { uid: 'mic-1', name: 'Built-in Microphone', transport: 'built_in', input_sample_rate_hz: 44100, input_channel_count: 1, read_back_uid: 'mic-1', first_frame_sample_count: 128, elapsed_ms: 12, result: 'compatible' },
  },
  identities: {
    app_version: '0.2.1', app_executable_sha256: 'a'.repeat(64), helper_code_sha256: 'b'.repeat(64),
    capture_protocol_version: 2, runtime_version: '1.13.5', runtime_git_commit: 'c'.repeat(40),
    runtime_archive_sha256: 'd'.repeat(64), runtime_build_id: 'sherpa-build',
  },
  models: [{ model_id: 'sense-voice-small', state: 'runtime_qualified', bundle_identity: 'e'.repeat(64), runtime_identity_sha256: 'f'.repeat(64), last_error_code: null }],
  storage: { data_directory: '/Users/test/Library/Application Support/LifeSub', catalog_bytes: 4096, audio_bytes: 32000, model_bytes: 240000000 },
  recent_error: { code: 'selected_microphone_disconnected', summary: '所选麦克风已断开' },
}

describe('DiagnosticsSettings', () => {
  beforeEach(() => {
    Object.values(mocks).forEach((mock) => mock.mockReset())
    mocks.load.mockResolvedValue(snapshot)
    mocks.exportSnapshot.mockResolvedValue('/tmp/lifesub-diagnostics.json')
    mocks.openPrivacy.mockResolvedValue(undefined)
    mocks.probe.mockResolvedValue(undefined)
    mocks.copy.mockReset()
    mocks.copy.mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: mocks.copy } })
  })

  it('renders permission, exact device, identities, model, storage and recent error facts', async () => {
    render(<DiagnosticsSettings />)

    expect(await screen.findByText('已授权')).toBeInTheDocument()
    expect(screen.getByText('未授权')).toBeInTheDocument()
    expect(screen.getByText('Built-in Microphone')).toBeInTheDocument()
    expect(screen.getByText('已通过首帧测试')).toBeInTheDocument()
    expect(screen.getByText('sense-voice-small')).toBeInTheDocument()
    expect(screen.getByText('e'.repeat(64))).toBeInTheDocument()
    expect(screen.getByText('f'.repeat(64))).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '复制sense-voice-small bundle identity' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '复制sense-voice-small runtime identity SHA-256' })).toBeInTheDocument()
    expect(screen.getByText('/Users/test/Library/Application Support/LifeSub')).toBeInTheDocument()
    expect(screen.getByText('selected_microphone_disconnected')).toBeInTheDocument()
    expect(screen.getByText('2 项状态需核对')).toBeInTheDocument()
  })

  it('summarizes a healthy snapshot without overstating certainty and copies exact facts', async () => {
    const user = userEvent.setup()
    Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: mocks.copy })
    mocks.load.mockResolvedValueOnce({
      ...snapshot,
      permissions: { microphone: 'granted', screen_recording: 'granted' },
      recent_error: null,
    })
    render(<DiagnosticsSettings />)

    expect(await screen.findByText('当前快照未见需核对项')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '复制麦克风 UID' }))
    await user.click(screen.getByRole('button', { name: '复制数据目录' }))
    await user.click(screen.getByRole('button', { name: '复制应用可执行文件 SHA-256' }))
    await user.click(screen.getByRole('button', { name: '复制麦克风设备类型' }))

    await waitFor(() => expect(mocks.copy).toHaveBeenCalledTimes(4))
    expect(mocks.copy).toHaveBeenNthCalledWith(1, 'mic-1')
    expect(mocks.copy).toHaveBeenNthCalledWith(2, '/Users/test/Library/Application Support/LifeSub')
    expect(mocks.copy).toHaveBeenNthCalledWith(3, 'a'.repeat(64))
    expect(mocks.copy).toHaveBeenNthCalledWith(4, 'built_in')
  })

  it('does not count optional uninstalled models as a setup problem', async () => {
    mocks.load.mockResolvedValueOnce({
      ...snapshot,
      permissions: { microphone: 'granted', screen_recording: 'granted' },
      models: [...snapshot.models, {
        model_id: 'optional-whisper', state: 'not_installed', bundle_identity: 'g'.repeat(64),
        runtime_identity_sha256: null, last_error_code: null,
      }],
      recent_error: null,
    })
    render(<DiagnosticsSettings />)
    expect(await screen.findByText('当前快照未见需核对项')).toBeInTheDocument()
  })

  it('discards a stale refresh and supports export plus allowlisted privacy actions', async () => {
    const user = userEvent.setup()
    let resolveOlder: ((value: typeof snapshot) => void) | undefined
    mocks.load
      .mockResolvedValueOnce(snapshot)
      .mockImplementationOnce(() => new Promise((resolve) => { resolveOlder = resolve }))
      .mockResolvedValueOnce({ ...snapshot, generated_at: '2026-08-27T00:01:00Z', recent_error: null })
    render(<DiagnosticsSettings />)
    await screen.findByText('selected_microphone_disconnected')

    fireEvent.click(screen.getByRole('button', { name: '刷新诊断' }))
    fireEvent.click(screen.getByRole('button', { name: '刷新诊断' }))
    await waitFor(() => expect(mocks.load).toHaveBeenCalledTimes(3))
    resolveOlder?.(snapshot)
    await waitFor(() => expect(screen.queryByText('selected_microphone_disconnected')).not.toBeInTheDocument())

    await user.click(screen.getByRole('button', { name: '导出诊断' }))
    await user.click(screen.getByRole('button', { name: '打开麦克风隐私设置' }))
    expect(mocks.exportSnapshot).toHaveBeenCalled()
    expect(mocks.openPrivacy).toHaveBeenCalledWith('microphone')
  })

  it('re-tests the exact selected microphone and refreshes the fact snapshot', async () => {
    const user = userEvent.setup()
    render(<DiagnosticsSettings />)
    await screen.findByText('Built-in Microphone')

    await user.click(screen.getByRole('button', { name: '重新测试选中麦克风' }))
    await waitFor(() => expect(mocks.probe).toHaveBeenCalledWith('mic-1'))
    await waitFor(() => expect(mocks.load).toHaveBeenCalledTimes(2))
  })

  it('shows load and export failures without stale success', async () => {
    const user = userEvent.setup()
    mocks.load.mockRejectedValueOnce(new Error('diagnostics unavailable'))
    render(<DiagnosticsSettings />)
    expect(await screen.findByText('diagnostics unavailable')).toBeInTheDocument()

    mocks.load.mockResolvedValueOnce(snapshot)
    await user.click(screen.getByRole('button', { name: '重试诊断' }))
    await screen.findByText('Built-in Microphone')
    mocks.exportSnapshot.mockRejectedValueOnce(new Error('export failed'))
    await user.click(screen.getByRole('button', { name: '导出诊断' }))
    expect(await screen.findByText('export failed')).toBeInTheDocument()
  })
})
