import { invoke } from '@tauri-apps/api/core'
import type { CaptureState } from '../domain'

export interface CoreCaptureSession {
  id: string
  title: string
  state: CaptureState
  started_at: string
  ended_at: string | null
}

export interface EvidenceResolution {
  kind: 'record' | 'segment' | 'audio'
  id: string
  start_seconds: number | null
  end_seconds: number | null
  revision: number | null
}

export interface CoreTranscriptSegment {
  id: string
  start_ms: number
  end_ms: number
  source: 'microphone' | 'system_audio' | 'imported'
  text: string
}

export interface CoreTranscriptRevision {
  id: string
  session_id: string
  number: number
  provider: string
  created_at: string
  provenance_status?: string
  receipt_ids?: string[]
  segments: CoreTranscriptSegment[]
}

export interface ImportResult {
  chunkId: string
  jobId: string | null
}

export interface CoreImportResult {
  chunk_id: string
  job_id: string | null
}

export interface CoreAsrJob {
  id: string
  session_id: string
  chunk_id: string
  provider: string
  model_id: string
  state: string
  error_code: string | null
  error_summary: string | null
  created_at: string
}

export interface AsrJobSummary {
  id: string
  sessionId: string
  chunkId: string
  provider: string
  modelId: string
  state: string
  errorCode: string | null
  errorSummary: string | null
  createdAt: string
}

export interface CoreProviderReceipt {
  id: string
  job_id: string
  chunk_id: string
  provider: string
  model_id: string
  manifest_version: string
  started_at: string
  finished_at: string
}

export interface ProviderReceiptSummary {
  id: string
  jobId: string
  chunkId: string
  provider: string
  modelId: string
  manifestVersion: string
  startedAt: string
  finishedAt: string
}

export function createCapture(title: string) {
  return invoke<CoreCaptureSession>('create_capture_session', { title })
}

export function transitionCapture(session: CoreCaptureSession, target: CaptureState) {
  return invoke<CoreCaptureSession>('transition_capture_session', { session, target })
}

export function importAudio(session: CoreCaptureSession, path: string) {
  return invoke<CoreImportResult>('import_audio_file', { session, path }).then(
    (result): ImportResult => ({ chunkId: result.chunk_id, jobId: result.job_id }),
  )
}

export function appendTranscriptRevision(sessionId: string, provider: string, segments: CoreTranscriptSegment[]) {
  return invoke<CoreTranscriptRevision>('append_transcript_revision', { sessionId, provider, segments })
}

export function resolveEvidence(uri: string) {
  return invoke<EvidenceResolution>('resolve_evidence', { uri })
}

export function listJobs(sessionId: string) {
  return invoke<CoreAsrJob[]>('list_asr_jobs', { sessionId }).then((jobs) =>
    jobs.map(
      (job): AsrJobSummary => ({
        id: job.id,
        sessionId: job.session_id,
        chunkId: job.chunk_id,
        provider: job.provider,
        modelId: job.model_id,
        state: job.state,
        errorCode: job.error_code,
        errorSummary: job.error_summary,
        createdAt: job.created_at,
      }),
    ),
  )
}

export function cancelJob(jobId: string) {
  return invoke<void>('cancel_asr_job', { jobId })
}

export function retryJob(jobId: string) {
  return invoke<CoreAsrJob>('retry_asr_job', { jobId }).then(
    (job): AsrJobSummary => ({
      id: job.id,
      sessionId: job.session_id,
      chunkId: job.chunk_id,
      provider: job.provider,
      modelId: job.model_id,
      state: job.state,
      errorCode: job.error_code,
      errorSummary: job.error_summary,
      createdAt: job.created_at,
    }),
  )
}

export function retranscribe(sessionId: string, chunkId: string) {
  return invoke<CoreAsrJob>('retranscribe_record', { sessionId, chunkId }).then(
    (job): AsrJobSummary => ({
      id: job.id,
      sessionId: job.session_id,
      chunkId: job.chunk_id,
      provider: job.provider,
      modelId: job.model_id,
      state: job.state,
      errorCode: job.error_code,
      errorSummary: job.error_summary,
      createdAt: job.created_at,
    }),
  )
}

export function getReceipt(jobId: string) {
  return invoke<CoreProviderReceipt>('get_provider_receipt', { jobId }).then(
    (receipt): ProviderReceiptSummary => ({
      id: receipt.id,
      jobId: receipt.job_id,
      chunkId: receipt.chunk_id,
      provider: receipt.provider,
      modelId: receipt.model_id,
      manifestVersion: receipt.manifest_version,
      startedAt: receipt.started_at,
      finishedAt: receipt.finished_at,
    }),
  )
}

export function getSessionRevision(sessionId: string) {
  return invoke<CoreTranscriptRevision>('get_session_revision', { sessionId }).then(
    (rev): CoreTranscriptRevision => ({
      ...rev,
      provenance_status: rev.provenance_status ?? 'legacy_unverified',
    }),
  )
}

export function isTauriRuntime() {
  return '__TAURI_INTERNALS__' in window
}