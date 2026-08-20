import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { AsrSettings, ModelInfo } from '../../domain'

const SENSE_VOICE_SETTINGS: AsrSettings = {
  provider: 'sense_voice',
  modelId: 'sense-voice-small-int8-2024-07-17',
  language: 'zh',
  numThreads: 4,
  vadEnabled: true,
  autoTranscribeImports: false,
  providerOptions: { kind: 'sense_voice', useItn: true },
}

const WHISPER_SETTINGS: AsrSettings = {
  provider: 'whisper',
  modelId: 'whisper-base',
  language: 'en',
  numThreads: 2,
  vadEnabled: false,
  autoTranscribeImports: false,
  providerOptions: { kind: 'whisper', task: 'transcribe' as const },
}

const SENSE_VOICE_MODEL: ModelInfo = {
  modelId: 'sense-voice-small-int8-2024-07-17',
  provider: 'sense_voice',
  displayName: 'SenseVoice Small INT8',
  description: 'Default Chinese/mixed model with ITN support',
  sizeBytes: 163_002_883,
  license: 'Apache-2.0',
  languages: ['zh', 'en', 'ja', 'ko', 'yue'],
  recommended: true,
  installed: false,
  downloadState: null,
}

const WHISPER_BASE_MODEL: ModelInfo = {
  modelId: 'whisper-base',
  provider: 'whisper',
  displayName: 'Whisper Base',
  description: 'Balanced accuracy and speed',
  sizeBytes: 207_557_382,
  license: 'MIT',
  languages: ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es'],
  recommended: true,
  installed: false,
  downloadState: null,
}

const WHISPER_TINY_MODEL: ModelInfo = {
  modelId: 'whisper-tiny',
  provider: 'whisper',
  displayName: 'Whisper Tiny',
  description: 'Fastest model for quick validation',
  sizeBytes: 116_204_861,
  license: 'MIT',
  languages: ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es'],
  recommended: false,
  installed: true,
  downloadState: null,
}

describe('AsrSettingsForm', () => {
  it('renders the SenseVoice/Whisper segmented control', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL, WHISPER_BASE_MODEL]}
        onSave={onSave}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    const senseVoiceButton = screen.getByRole('radio', { name: /sensevoice/i })
    const whisperButton = screen.getByRole('radio', { name: /whisper/i })
    expect(senseVoiceButton).toBeInTheDocument()
    expect(whisperButton).toBeInTheDocument()
  })

  it('renders compatible model cards for the selected provider', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL, WHISPER_BASE_MODEL]}
        onSave={onSave}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByText('SenseVoice Small INT8')).toBeInTheDocument()
    expect(screen.queryByText('Whisper Base')).not.toBeInTheDocument()
  })

  it('shows download button for uninstalled models', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onDownloadModel = vi.fn().mockResolvedValue(undefined)
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[{ ...SENSE_VOICE_MODEL, installed: false, downloadState: null }]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={onDownloadModel}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    const downloadButton = screen.getByRole('button', { name: /download/i })
    expect(downloadButton).toBeInTheDocument()

    await userEvent.setup().click(downloadButton)
    expect(onDownloadModel).toHaveBeenCalledWith('sense-voice-small-int8-2024-07-17')
  })

  it('shows cancel button for downloading models', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onCancelDownload = vi.fn().mockResolvedValue(undefined)
    const downloadingModel: ModelInfo = {
      ...SENSE_VOICE_MODEL,
      installed: false,
      downloadState: {
        id: 'dl_1',
        modelId: 'sense-voice-small-int8-2024-07-17',
        state: 'downloading',
        downloadedBytes: 50_000_000,
        expectedBytes: 163_002_883,
        errorCode: null,
      },
    }
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[downloadingModel]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={onCancelDownload}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    const cancelButton = screen.getByRole('button', { name: /cancel/i })
    expect(cancelButton).toBeInTheDocument()

    await userEvent.setup().click(cancelButton)
    expect(onCancelDownload).toHaveBeenCalledWith('dl_1')
  })

  it('shows delete button for installed models', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onDeleteModel = vi.fn().mockResolvedValue(undefined)
    render(
      <AsrSettingsForm
        settings={WHISPER_SETTINGS}
        models={[WHISPER_TINY_MODEL, WHISPER_BASE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={onDeleteModel}
        loading={false}
        error={null}
      />,
    )

    const deleteButton = screen.getByRole('button', { name: /delete/i })
    expect(deleteButton).toBeInTheDocument()

    await userEvent.setup().click(deleteButton)
    expect(onDeleteModel).toHaveBeenCalledWith('whisper-tiny')
  })

  it('renders the language selector', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByLabelText(/language/i)).toBeInTheDocument()
  })

  it('renders a thread stepper', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByLabelText(/thread/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/thread/i)).toHaveValue(4)
  })

  it('renders VAD and auto-transcribe toggles', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByLabelText(/VAD/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/auto.transcribe/i)).toBeInTheDocument()
  })

  it('shows SenseVoice ITN toggle when SenseVoice is selected', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByLabelText(/ITN/i)).toBeInTheDocument()
  })

  it('shows Whisper task segmented control when Whisper is selected', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={WHISPER_SETTINGS}
        models={[WHISPER_TINY_MODEL, WHISPER_BASE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByRole('radio', { name: /transcribe/i })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /translate/i })).toBeInTheDocument()
  })

  it('calls onSave with updated settings when save is clicked', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onSave = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={onSave}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    await user.click(screen.getByRole('button', { name: /save/i }))

    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: 'sense_voice',
        modelId: 'sense-voice-small-int8-2024-07-17',
      }),
    )
  })

  it('displays save errors', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error="无法保存设置：磁盘空间不足"
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent(/磁盘空间不足/)
  })

  it('renders a fixed loading layout when loading', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    render(
      <AsrSettingsForm
        settings={null}
        models={[]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={true}
        error={null}
      />,
    )

    const loadingElement = screen.getByRole('status')
    expect(loadingElement).toBeInTheDocument()
    expect(loadingElement).toHaveAttribute('aria-label', expect.stringMatching(/loading/i))
  })

  it('shows download progress for active downloads', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const downloadingModel: ModelInfo = {
      ...SENSE_VOICE_MODEL,
      installed: false,
      downloadState: {
        id: 'dl_1',
        modelId: 'sense-voice-small-int8-2024-07-17',
        state: 'downloading',
        downloadedBytes: 80_000_000,
        expectedBytes: 163_002_883,
        errorCode: null,
      },
    }
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[downloadingModel]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByText(/80\.0 MB \/ 163\.0 MB/i)).toBeInTheDocument()
  })

  it('shows download error state', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const failedModel: ModelInfo = {
      ...SENSE_VOICE_MODEL,
      installed: false,
      downloadState: {
        id: 'dl_1',
        modelId: 'sense-voice-small-int8-2024-07-17',
        state: 'failed',
        downloadedBytes: 10_000_000,
        expectedBytes: 163_002_883,
        errorCode: 'model_download_failed',
      },
    }
    render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[failedModel]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByText(/model_download_failed/i)).toBeInTheDocument()
  })

  it('switches models when provider changes', async () => {
    const { AsrSettingsForm } = await import('./AsrSettingsForm')
    const onSave = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    const { rerender } = render(
      <AsrSettingsForm
        settings={SENSE_VOICE_SETTINGS}
        models={[SENSE_VOICE_MODEL, WHISPER_BASE_MODEL, WHISPER_TINY_MODEL]}
        onSave={onSave}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    await user.click(screen.getByRole('radio', { name: /whisper/i }))

    // Re-render with Whisper settings after the provider change callback
    rerender(
      <AsrSettingsForm
        settings={WHISPER_SETTINGS}
        models={[SENSE_VOICE_MODEL, WHISPER_BASE_MODEL, WHISPER_TINY_MODEL]}
        onSave={onSave}
        onDownloadModel={vi.fn()}
        onCancelDownload={vi.fn()}
        onDeleteModel={vi.fn()}
        loading={false}
        error={null}
      />,
    )

    expect(screen.getByText('Whisper Base')).toBeInTheDocument()
    expect(screen.getByText('Whisper Tiny')).toBeInTheDocument()
    expect(screen.queryByText('SenseVoice Small INT8')).not.toBeInTheDocument()
  })
})