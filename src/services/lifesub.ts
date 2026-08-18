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

export interface CoreNote {
  id: string
  session_id: string
  content: string
  timestamp_ms: number
  tag: string
  segment_id: string | null
  created_at: string
}

export interface CoreCategory {
  id: string
  name: string
  scope: string
  entry_count: number
}

export interface CoreEntry {
  id: string
  category_id: string
  term: string
  pinyin: string
  aliases: string
  note: string
  enabled: boolean
}

export interface CoreVoiceprint {
  id: string
  name: string
  embedding_path: string
  dictionary_entry_id: string | null
  sample_count: number
  updated_at: string
}

export interface CoreStats {
  hourly_slots: Array<{
    hour: number
    minutes: number
    session_id: string | null
    title: string | null
  }>
  week_sessions: number
  week_minutes: number
  month_sessions: number
  month_minutes: number
  total_sessions: number
  total_minutes: number
}

export interface CoreAsrConfig {
  provider: string
  language: string
  auto_transcribe: boolean
  threads: number
  vad_enabled: boolean
  vad_min_speech_ms: number
  vad_silence_ms: number
  itn_enabled: boolean
}

export interface CoreRecordingConfig {
  capture_mode: string
  im_detection_enabled: boolean
  im_apps: string[]
  detection_delay_secs: number
  recovery_delay_secs: number
  sample_rate: number
  storage_path: string
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

// ── Task 13.5: Notes ─────────────────────────────────────────────────────

export function createNote(sessionId: string, content: string, timestampMs: number, tag: string, segmentId: string | null) {
  return invoke<CoreNote>('create_note', { sessionId, content, timestampMs, tag, segmentId })
}

export function listNotes(sessionId: string) {
  return invoke<CoreNote[]>('list_notes', { sessionId })
}

export function updateNote(noteId: string, content: string, tag: string) {
  return invoke('update_note', { noteId, content, tag })
}

export function deleteNote(noteId: string) {
  return invoke('delete_note', { noteId })
}

// ── Task 13.5: Dictionary ────────────────────────────────────────────────

export function createCategory(name: string, scope: string) {
  return invoke<CoreCategory>('create_category', { name, scope })
}

export function listCategories(scope?: string) {
  return invoke<CoreCategory[]>('list_categories', { scope: scope ?? null })
}

export function deleteCategory(categoryId: string) {
  return invoke('delete_category', { categoryId })
}

export function createEntry(categoryId: string, term: string, pinyin: string, aliases: string, note: string) {
  return invoke<CoreEntry>('create_entry', { categoryId, term, pinyin, aliases, note })
}

export function listEntries(categoryId: string, query?: string) {
  return invoke<CoreEntry[]>('list_entries', { categoryId, query: query ?? null })
}

export function updateEntry(entryId: string, term: string, pinyin: string, aliases: string, note: string) {
  return invoke('update_entry', { entryId, term, pinyin, aliases, note })
}

export function toggleEntry(entryId: string, enabled: boolean) {
  return invoke('toggle_entry', { entryId, enabled })
}

export function deleteEntry(entryId: string) {
  return invoke('delete_entry', { entryId })
}

// ── Task 13.5: Voiceprints ───────────────────────────────────────────────

export function listVoiceprints() {
  return invoke<CoreVoiceprint[]>('list_voiceprints')
}

export function registerVoiceprint(name: string, embeddingPath: string, dictionaryEntryId: string | null) {
  return invoke<CoreVoiceprint>('register_voiceprint', { name, embeddingPath, dictionaryEntryId })
}

export function renameVoiceprint(voiceprintId: string, name: string) {
  return invoke('rename_voiceprint', { voiceprintId, name })
}

export function deleteVoiceprint(voiceprintId: string) {
  return invoke('delete_voiceprint', { voiceprintId })
}

export function linkVoiceprintToEntry(voiceprintId: string, entryId: string) {
  return invoke('link_voiceprint_to_entry', { voiceprintId, entryId })
}

// ── Task 13.5: Stats & Settings ──────────────────────────────────────────

export function getStatsSnapshot(date?: string) {
  return invoke<CoreStats>('get_stats_snapshot', { date: date ?? null })
}

export function getAsrConfig() {
  return invoke<CoreAsrConfig>('get_asr_config')
}

export function setAsrConfig(config: CoreAsrConfig) {
  return invoke('set_asr_config', { config })
}

export function getRecordingConfig() {
  return invoke<CoreRecordingConfig>('get_recording_config')
}

export function setRecordingConfig(config: CoreRecordingConfig) {
  return invoke('set_recording_config', { config })
}

// ── Phase 2.1: Streaming capture ─────────────────────────────────────────

export function startStreamingCapture() {
  return invoke('start_streaming_capture')
}

export function stopStreamingCapture() {
  return invoke('stop_streaming_capture')
}

export function pauseStreamingCapture() {
  return invoke('pause_streaming_capture')
}

export function resumeStreamingCapture() {
  return invoke('resume_streaming_capture')
}