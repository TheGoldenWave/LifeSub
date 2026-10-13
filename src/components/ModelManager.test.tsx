import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  install: vi.fn(), cancel: vi.fn(), retry: vi.fn(), refresh: vi.fn(),
  state: {} as any,
}))

vi.mock('../modelManager', () => ({
  useModelManagerController: () => ({ ...mocks.state, install: mocks.install, cancel: mocks.cancel, retry: mocks.retry, refresh: mocks.refresh }),
}))

import { ModelManager } from './ModelManager'

const baseModel = {
  model_id: 'sense-voice-small', display_name: 'SenseVoice Small', provider: 'sense_voice',
  manifest_version: '1', bundle_identity: 'a'.repeat(64), supported_languages: ['auto', 'zh'],
  qualification_policy: 'structural_with_pinned_runtime', runtime_family: 'sherpa_onnx', runtime_version: '1.13.5',
  artifact_count: 2, total_bytes: 100, license_spdx: 'MIT', installation_state: 'not_installed',
  selectable: false, installable: true, executable: false, reason_code: 'model_not_installed', last_error_code: null,
}

describe('ModelManager', () => {
  beforeEach(() => {
    Object.values(mocks).filter((value) => typeof value === 'function').forEach((mock: any) => mock.mockReset())
    mocks.state = { models: [], loading: false, refreshing: false, error: null, pendingModelId: null, cancelRequestedDownloadId: null }
  })

  it('shows durable byte, stage and artifact progress and requests cancellation', async () => {
    const user = userEvent.setup()
    mocks.state = {
      ...mocks.state,
      pendingModelId: 'sense-voice-small',
      models: [{
        ...baseModel,
        download: {
          id: 'mdl_1', state: 'downloading', downloaded_bytes: 50, expected_bytes: 100,
          artifact_index: 1, artifact_count: 2, error_code: null, error_summary: null,
        },
      }],
    }
    render(<ModelManager />)

    expect(screen.getByRole('progressbar', { name: 'SenseVoice Small 下载进度' })).toHaveAttribute('aria-valuenow', '50')
    expect(screen.getByText('50 B / 100 B')).toBeInTheDocument()
    expect(screen.getByText('下载中 · 文件 1/2')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '取消 SenseVoice Small 下载' }))
    expect(mocks.cancel).toHaveBeenCalledWith('sense-voice-small', 'mdl_1')
  })

  it('projects the persisted verifying stage into a four-step installation timeline', () => {
    mocks.state.models = [{
      ...baseModel,
      download: {
        id: 'mdl_verify', state: 'verifying', downloaded_bytes: 100, expected_bytes: 100,
        artifact_index: 2, artifact_count: 2, error_code: null, error_summary: null,
      },
    }]
    render(<ModelManager />)

    const timeline = screen.getByRole('list', { name: 'SenseVoice Small 安装步骤' })
    expect(within(timeline).getByText('下载文件').closest('li')).toHaveAttribute('data-stage', 'done')
    expect(within(timeline).getByText('校验文件').closest('li')).toHaveAttribute('data-stage', 'current')
    expect(within(timeline).getByText('安装文件').closest('li')).toHaveAttribute('data-stage', 'pending')
    expect(within(timeline).getByText('运行时验证').closest('li')).toHaveAttribute('data-stage', 'pending')
  })

  it('retries the same failed download and exposes stable error details', async () => {
    const user = userEvent.setup()
    mocks.state.models = [{
      ...baseModel,
      download: {
        id: 'mdl_failed', state: 'failed', downloaded_bytes: 40, expected_bytes: 100,
        artifact_index: 1, artifact_count: 2, error_code: 'model_download_failed', error_summary: 'network interrupted',
      },
    }]
    render(<ModelManager />)

    expect(screen.getByText('下载失败')).toBeInTheDocument()
    expect(screen.getByText('network interrupted')).toBeInTheDocument()
    expect(screen.getByText('model_download_failed')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重试 SenseVoice Small 下载' }))
    expect(mocks.retry).toHaveBeenCalledWith('sense-voice-small', 'mdl_failed')
  })

  it('derives one primary status while keeping the action separate', () => {
    mocks.state.models = [{
      ...baseModel,
      installation_state: 'installed_unqualified',
      reason_code: 'runtime_not_qualified',
      last_error_code: 'runtime_gate_failed',
    }]
    render(<ModelManager />)

    expect(screen.getByText('运行时验证失败')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '安装 SenseVoice Small' })).toBeInTheDocument()
    expect(screen.getByText('runtime_gate_failed')).toBeInTheDocument()
  })

  it('does not repeat unavailable installation as a second status', () => {
    mocks.state.models = [{ ...baseModel, installable: false }]
    render(<ModelManager />)

    expect(screen.getAllByText('暂不可安装')).toHaveLength(1)
    expect(screen.queryByText('未安装')).not.toBeInTheDocument()
  })

  it('explains when model management requires the desktop app', () => {
    mocks.state.models = [{ ...baseModel, installable: false, reason_code: 'desktop_runtime_required' }]
    render(<ModelManager />)

    expect(screen.getByText('请在 LifeSub 桌面应用中管理和安装本地模型。')).toBeInTheDocument()
  })
})
