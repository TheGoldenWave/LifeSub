import { invoke } from '@tauri-apps/api/core'
import type {
  AsrJob,
  AsrJobCore,
  AsrSettings,
  AsrSettingsCore,
  ModelDownload,
  ModelDownloadCore,
  ModelInfo,
  ModelInfoCore,
  ProviderReceipt,
  ProviderReceiptCore,
} from '../domain'

// -- snake_case ↔ camelCase mapping (single place) --

function mapSettingsToCore(settings: AsrSettings): AsrSettingsCore {
  const providerOptions =
    settings.providerOptions.kind === 'sense_voice'
      ? { kind: 'sense_voice' as const, use_itn: settings.providerOptions.useItn }
      : { kind: 'whisper' as const, task: settings.providerOptions.task }
  return {
    provider: settings.provider,
    model_id: settings.modelId,
    language: settings.language,
    num_threads: settings.numThreads,
    vad_enabled: settings.vadEnabled,
    // auto_transcribe_imports is not yet surfaced in the UI; keep false
    auto_transcribe_imports: false,
    provider_options: providerOptions,
  }
}

function mapSettingsFromCore(core: AsrSettingsCore): AsrSettings {
  return {
    provider: core.provider,
    modelId: core.model_id,
    language: core.language,
    numThreads: core.num_threads,
    vadEnabled: core.vad_enabled,
    autoTranscribeImports: core.auto_transcribe_imports,
    providerOptions:
      core.provider_options.kind === 'sense_voice'
        ? { kind: 'sense_voice', useItn: core.provider_options.use_itn }
        : { kind: 'whisper', task: core.provider_options.task },
  }
}

function mapModelFromCore(core: ModelInfoCore): ModelInfo {
  return {
    modelId: core.model_id,
    provider: core.provider,
    displayName: core.display_name,
    description: core.description,
    sizeBytes: core.size_bytes,
    license: core.license,
    languages: core.languages,
    recommended: core.recommended,
    installed: core.installed,
    downloadState: core.download_state
      ? {
          id: core.download_state.id,
          modelId: core.download_state.model_id,
          state: core.download_state.state,
          downloadedBytes: core.download_state.downloaded_bytes,
          expectedBytes: core.download_state.expected_bytes,
          errorCode: core.download_state.error_code,
        }
      : null,
  }
}

function mapDownloadFromCore(core: ModelDownloadCore): ModelDownload {
  return {
    id: core.id,
    modelId: core.model_id,
    state: core.state,
    downloadedBytes: core.downloaded_bytes,
    expectedBytes: core.expected_bytes,
    errorCode: core.error_code,
  }
}

function mapJobFromCore(core: AsrJobCore): AsrJob {
  return {
    id: core.id,
    sessionId: core.session_id,
    chunkId: core.chunk_id,
    provider: core.provider,
    modelId: core.model_id,
    state: core.state,
    errorCode: core.error_code,
    errorSummary: core.error_summary,
    createdAt: core.created_at,
  }
}

function mapReceiptFromCore(core: ProviderReceiptCore): ProviderReceipt {
  return {
    id: core.id,
    jobId: core.job_id,
    chunkId: core.chunk_id,
    provider: core.provider,
    modelId: core.model_id,
    manifestVersion: core.manifest_version,
    startedAt: core.started_at,
    finishedAt: core.finished_at,
  }
}

// -- public API --

export async function getAsrSettings(): Promise<AsrSettings> {
  const core = await invoke<AsrSettingsCore>('get_asr_settings')
  return mapSettingsFromCore(core)
}

export async function saveAsrSettings(settings: AsrSettings): Promise<AsrSettings> {
  const core = await invoke<AsrSettingsCore>('save_asr_settings', {
    settings: mapSettingsToCore(settings),
  })
  return mapSettingsFromCore(core)
}

export async function listAsrModels(): Promise<ModelInfo[]> {
  const core = await invoke<ModelInfoCore[]>('list_asr_models')
  return core.map(mapModelFromCore)
}

export async function downloadAsrModel(modelId: string): Promise<ModelDownload> {
  const core = await invoke<ModelDownloadCore>('download_asr_model', { modelId })
  return mapDownloadFromCore(core)
}

export async function cancelModelDownload(downloadId: string): Promise<void> {
  await invoke('cancel_model_download', { downloadId })
}

export async function deleteAsrModel(modelId: string): Promise<void> {
  await invoke('delete_asr_model', { modelId })
}

export async function listAsrJobs(): Promise<AsrJob[]> {
  const core = await invoke<AsrJobCore[]>('list_asr_jobs')
  return core.map(mapJobFromCore)
}

export async function cancelAsrJob(jobId: string): Promise<void> {
  await invoke('cancel_asr_job', { jobId })
}

export async function retryAsrJob(jobId: string): Promise<AsrJob> {
  const core = await invoke<AsrJobCore>('retry_asr_job', { jobId })
  return mapJobFromCore(core)
}

export async function retranscribeRecord(sessionId: string, chunkId: string): Promise<AsrJob> {
  const core = await invoke<AsrJobCore>('retranscribe_record', { sessionId, chunkId })
  return mapJobFromCore(core)
}

export async function getProviderReceipt(jobId: string): Promise<ProviderReceipt> {
  const core = await invoke<ProviderReceiptCore>('get_provider_receipt', { jobId })
  return mapReceiptFromCore(core)
}

export function isTauriRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}