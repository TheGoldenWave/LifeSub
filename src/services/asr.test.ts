import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const DEFAULT_SETTINGS_CORE = {
  provider: 'sense_voice',
  model_id: 'sense-voice-small-int8-2024-07-17',
  language: 'zh',
  num_threads: 4,
  vad_enabled: true,
  auto_transcribe_imports: false,
  provider_options: { kind: 'sense_voice', use_itn: true },
}

const DEFAULT_MODEL_CORE = {
  model_id: 'sense-voice-small-int8-2024-07-17',
  provider: 'sense_voice',
  display_name: 'SenseVoice Small INT8',
  description: 'Default Chinese/mixed model',
  size_bytes: 163_002_883,
  license: 'Apache-2.0',
  languages: ['zh', 'en', 'ja', 'ko', 'yue'],
  recommended: true,
  installed: false,
  download_state: null,
}

const DEFAULT_MODEL_DOWNLOAD_CORE = {
  id: 'dl_1',
  model_id: 'sense-voice-small-int8-2024-07-17',
  manifest_version: '2024-07-17',
  state: 'downloading',
  downloaded_bytes: 50_000_000,
  expected_bytes: 163_002_883,
  error_code: null,
}

const DEFAULT_JOB_CORE = {
  id: 'job_1',
  session_id: 'rec_1',
  chunk_id: 'chk_1',
  provider: 'sense_voice',
  model_id: 'sense-voice-small-int8-2024-07-17',
  state: 'queued',
  error_code: null,
  error_summary: null,
  created_at: '2026-08-15T00:00:00Z',
}

const DEFAULT_RECEIPT_CORE = {
  id: 'rcp_1',
  job_id: 'job_1',
  chunk_id: 'chk_1',
  provider: 'sense_voice',
  model_id: 'sense-voice-small-int8-2024-07-17',
  manifest_version: '2024-07-17',
  started_at: '2026-08-15T00:00:01Z',
  finished_at: '2026-08-15T00:00:05Z',
}

describe('ASR desktop client', () => {
  beforeEach(() => {
    invoke.mockReset()
  })

  describe('settings commands', () => {
    it('maps getAsrSettings to the Tauri command get_asr_settings', async () => {
      const { getAsrSettings } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_SETTINGS_CORE)

      const result = await getAsrSettings()

      expect(invoke).toHaveBeenCalledWith('get_asr_settings')
      expect(result.provider).toBe('sense_voice')
      expect(result.modelId).toBe('sense-voice-small-int8-2024-07-17')
      expect(result.language).toBe('zh')
      expect(result.numThreads).toBe(4)
      expect(result.vadEnabled).toBe(true)
      expect(result.autoTranscribeImports).toBe(false)
      expect(result.providerOptions).toEqual({ kind: 'sense_voice', useItn: true })
    })

    it('maps saveAsrSettings to the Tauri command save_asr_settings', async () => {
      const { saveAsrSettings } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_SETTINGS_CORE)

      const settings = {
        provider: 'whisper' as const,
        modelId: 'whisper-base',
        language: 'en',
        numThreads: 2,
        vadEnabled: false,
        autoTranscribeImports: true,
        providerOptions: { kind: 'whisper' as const, task: 'transcribe' as const },
      }
      await saveAsrSettings(settings)

      expect(invoke).toHaveBeenCalledWith('save_asr_settings', {
        settings: {
          provider: 'whisper',
          model_id: 'whisper-base',
          language: 'en',
          num_threads: 2,
          vad_enabled: false,
          auto_transcribe_imports: false,
          provider_options: { kind: 'whisper', task: 'transcribe' },
        },
      })
    })

    it('persists auto_transcribe_imports as false when saveAsrSettings is called with true', async () => {
      const { saveAsrSettings } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_SETTINGS_CORE)

      await saveAsrSettings({
        provider: 'sense_voice',
        modelId: 'sense-voice-small-int8-2024-07-17',
        language: 'zh',
        numThreads: 4,
        vadEnabled: true,
        autoTranscribeImports: true,
        providerOptions: { kind: 'sense_voice', useItn: true },
      })

      const callArgs = invoke.mock.calls[0][1]
      expect(callArgs.settings.auto_transcribe_imports).toBe(false)
    })
  })

  describe('model commands', () => {
    it('maps listAsrModels to the Tauri command list_asr_models', async () => {
      const { listAsrModels } = await import('./asr')
      invoke.mockResolvedValue([DEFAULT_MODEL_CORE])

      const result = await listAsrModels()

      expect(invoke).toHaveBeenCalledWith('list_asr_models')
      expect(result).toHaveLength(1)
      expect(result[0].modelId).toBe('sense-voice-small-int8-2024-07-17')
      expect(result[0].displayName).toBe('SenseVoice Small INT8')
      expect(result[0].sizeBytes).toBe(163_002_883)
      expect(result[0].languages).toEqual(['zh', 'en', 'ja', 'ko', 'yue'])
      expect(result[0].recommended).toBe(true)
      expect(result[0].installed).toBe(false)
      expect(result[0].downloadState).toBeNull()
    })

    it('maps downloadAsrModel to the Tauri command download_asr_model', async () => {
      const { downloadAsrModel } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_MODEL_DOWNLOAD_CORE)

      const result = await downloadAsrModel('sense-voice-small-int8-2024-07-17')

      expect(invoke).toHaveBeenCalledWith('download_asr_model', {
        modelId: 'sense-voice-small-int8-2024-07-17',
      })
      expect(result.id).toBe('dl_1')
      expect(result.state).toBe('downloading')
      expect(result.downloadedBytes).toBe(50_000_000)
      expect(result.expectedBytes).toBe(163_002_883)
    })

    it('maps cancelModelDownload to the Tauri command cancel_model_download', async () => {
      const { cancelModelDownload } = await import('./asr')
      invoke.mockResolvedValue(undefined)

      await cancelModelDownload('dl_1')

      expect(invoke).toHaveBeenCalledWith('cancel_model_download', {
        downloadId: 'dl_1',
      })
    })

    it('maps deleteAsrModel to the Tauri command delete_asr_model', async () => {
      const { deleteAsrModel } = await import('./asr')
      invoke.mockResolvedValue(undefined)

      await deleteAsrModel('sense-voice-small-int8-2024-07-17')

      expect(invoke).toHaveBeenCalledWith('delete_asr_model', {
        modelId: 'sense-voice-small-int8-2024-07-17',
      })
    })
  })

  describe('job commands', () => {
    it('maps listAsrJobs to the Tauri command list_asr_jobs', async () => {
      const { listAsrJobs } = await import('./asr')
      invoke.mockResolvedValue([DEFAULT_JOB_CORE])

      const result = await listAsrJobs()

      expect(invoke).toHaveBeenCalledWith('list_asr_jobs')
      expect(result).toHaveLength(1)
      expect(result[0].id).toBe('job_1')
      expect(result[0].sessionId).toBe('rec_1')
      expect(result[0].chunkId).toBe('chk_1')
      expect(result[0].provider).toBe('sense_voice')
      expect(result[0].state).toBe('queued')
    })

    it('maps cancelAsrJob to the Tauri command cancel_asr_job', async () => {
      const { cancelAsrJob } = await import('./asr')
      invoke.mockResolvedValue(undefined)

      await cancelAsrJob('job_1')

      expect(invoke).toHaveBeenCalledWith('cancel_asr_job', {
        jobId: 'job_1',
      })
    })

    it('maps retryAsrJob to the Tauri command retry_asr_job', async () => {
      const { retryAsrJob } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_JOB_CORE)

      const result = await retryAsrJob('job_1')

      expect(invoke).toHaveBeenCalledWith('retry_asr_job', {
        jobId: 'job_1',
      })
      expect(result.state).toBe('queued')
    })

    it('maps retranscribeRecord to the Tauri command retranscribe_record', async () => {
      const { retranscribeRecord } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_JOB_CORE)

      const result = await retranscribeRecord('rec_1', 'chk_1')

      expect(invoke).toHaveBeenCalledWith('retranscribe_record', {
        sessionId: 'rec_1',
        chunkId: 'chk_1',
      })
      expect(result.sessionId).toBe('rec_1')
    })
  })

  describe('receipt query', () => {
    it('maps getProviderReceipt to the Tauri command get_provider_receipt', async () => {
      const { getProviderReceipt } = await import('./asr')
      invoke.mockResolvedValue(DEFAULT_RECEIPT_CORE)

      const result = await getProviderReceipt('job_1')

      expect(invoke).toHaveBeenCalledWith('get_provider_receipt', {
        jobId: 'job_1',
      })
      expect(result.jobId).toBe('job_1')
      expect(result.provider).toBe('sense_voice')
      expect(result.modelId).toBe('sense-voice-small-int8-2024-07-17')
    })
  })

  describe('runtime detection', () => {
    it('returns false from isTauriRuntime when the Tauri API is absent', async () => {
      const { isTauriRuntime } = await import('./asr')
      expect(isTauriRuntime()).toBe(false)
    })
  })
})