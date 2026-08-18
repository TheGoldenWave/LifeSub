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
  segments: CoreTranscriptSegment[]
}

export function createCapture(title: string) {
  return invoke<CoreCaptureSession>('create_capture_session', { title })
}

export function transitionCapture(session: CoreCaptureSession, target: CaptureState) {
  return invoke<CoreCaptureSession>('transition_capture_session', { session, target })
}

export function importAudio(session: CoreCaptureSession, path: string) {
  return invoke('import_audio_file', { session, path })
}

export function appendTranscriptRevision(sessionId: string, provider: string, segments: CoreTranscriptSegment[]) {
  return invoke<CoreTranscriptRevision>('append_transcript_revision', { sessionId, provider, segments })
}

export function resolveEvidence(uri: string) {
  return invoke<EvidenceResolution>('resolve_evidence', { uri })
}

export function isTauriRuntime() {
  return '__TAURI_INTERNALS__' in window
}
