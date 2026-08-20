export type CaptureState = 'idle' | 'recording' | 'paused' | 'stopped'

export interface TranscriptSegment {
  id: string
  startMs: number
  endMs: number
  source: '麦克风' | '系统音频' | '导入音频'
  text: string
}

export interface TranscriptRevision {
  number: number
  provider: string
  label: string
  segments: TranscriptSegment[]
  /** V0.2: provenance status for the revision */
  provenance?: 'legacy_unverified' | 'verified_local_asr' | 'manual'
  /** V0.2: IDs of associated provider receipts */
  receiptIds?: string[]
}

export interface EvidenceRecord {
  id: string
  title: string
  startedAt: string
  duration: string
  status: 'available' | 'processing'
  revision: TranscriptRevision
  originalRevision: TranscriptRevision
  /** V0.2: all revisions for this record (for revision selector) */
  allRevisions?: TranscriptRevision[]
  /** V0.2: current chunk integrity state */
  chunkIntegrity?: 'available' | 'corrupted' | 'missing'
}

// -- ASR domain types (V0.2) --

export type AsrProviderKind = 'sense_voice' | 'whisper'

export type WhisperTask = 'transcribe' | 'translate'

/** camelCase view model — the only shape UI components import. */
export type AsrProviderOptions =
  | { kind: 'sense_voice'; useItn: boolean }
  | { kind: 'whisper'; task: WhisperTask }

export interface AsrSettings {
  provider: AsrProviderKind
  modelId: string
  language: string
  numThreads: number
  vadEnabled: boolean
  autoTranscribeImports: boolean
  providerOptions: AsrProviderOptions
}

/** snake_case Core DTO — only used inside the ASR service boundary. */
export interface AsrSettingsCore {
  provider: AsrProviderKind
  model_id: string
  language: string
  num_threads: number
  vad_enabled: boolean
  auto_transcribe_imports: boolean
  provider_options: AsrProviderOptionsCore
}

export type AsrProviderOptionsCore =
  | { kind: 'sense_voice'; use_itn: boolean }
  | { kind: 'whisper'; task: WhisperTask }

export type ModelDownloadState =
  | 'queued'
  | 'downloading'
  | 'verifying'
  | 'installing'
  | 'succeeded'
  | 'failed'
  | 'cancelled'

export interface ModelDownload {
  id: string
  modelId: string
  state: ModelDownloadState
  downloadedBytes: number
  expectedBytes: number
  errorCode: string | null
}

export interface ModelDownloadCore {
  id: string
  model_id: string
  manifest_version: string
  state: ModelDownloadState
  downloaded_bytes: number
  expected_bytes: number
  error_code: string | null
}

export interface ModelInfo {
  modelId: string
  provider: AsrProviderKind
  displayName: string
  description: string
  sizeBytes: number
  license: string
  languages: string[]
  recommended: boolean
  installed: boolean
  downloadState: ModelDownload | null
}

export interface ModelInfoCore {
  model_id: string
  provider: AsrProviderKind
  display_name: string
  description: string
  size_bytes: number
  license: string
  languages: string[]
  recommended: boolean
  installed: boolean
  download_state: ModelDownloadCore | null
}

export type AsrJobState =
  | 'queued'
  | 'blocked_model'
  | 'preparing'
  | 'transcribing'
  | 'succeeded'
  | 'failed'
  | 'cancelled'

export interface AsrJob {
  id: string
  sessionId: string
  chunkId: string
  provider: AsrProviderKind
  modelId: string
  state: AsrJobState
  errorCode: string | null
  errorSummary: string | null
  createdAt: string
}

export interface AsrJobCore {
  id: string
  session_id: string
  chunk_id: string
  provider: AsrProviderKind
  model_id: string
  state: AsrJobState
  error_code: string | null
  error_summary: string | null
  created_at: string
}

export interface ProviderReceipt {
  id: string
  jobId: string
  chunkId: string
  provider: AsrProviderKind
  modelId: string
  manifestVersion: string
  startedAt: string
  finishedAt: string
}

export interface ProviderReceiptCore {
  id: string
  job_id: string
  chunk_id: string
  provider: AsrProviderKind
  model_id: string
  manifest_version: string
  started_at: string
  finished_at: string
}
