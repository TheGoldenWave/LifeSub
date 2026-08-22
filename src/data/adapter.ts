import type { DictionaryCategory, DictionaryEntry, StatsSnapshot, CaptureNote, EvidenceRecord, TranscriptRevision, TranscriptSegment } from '../domain'
import {
  createManualRevision,
  importAudioRecord as importAudioRecordCommand,
  isTauriRuntime,
  listTimelineRecords,
  listCategories,
  listEntries,
  createCategory,
  deleteCategory,
  createEntry,
  updateEntry,
  toggleEntry,
  deleteEntry,
  getStatsSnapshot,
  listVoiceprints,
  getAsrConfig,
  setAsrConfig,
  getRecordingConfig,
  setRecordingConfig,
  getAppRuntimeInfo,
  listAsrModels,
  renameVoiceprint,
  deleteVoiceprint,
  listNotes,
  createNote,
  updateNote,
  deleteNote,
  type CoreCategory,
  type CoreEntry,
  type CoreStats,
  type CoreVoiceprint,
  type CoreAsrConfig,
  type CoreRecordingConfig,
  type CoreNote,
  type CoreTimelineRecord,
  type CoreTranscriptSegment,
  type AppRuntimeInfo,
  type CoreAsrModel,
} from '../services/lifesub'
import {
  demoCategories,
  demoEntries,
  demoRecords,
  demoStats,
  demoVoiceprints,
  demoNotes,
} from './demo'

// ── Helpers ──────────────────────────────────────────────────────────────

function coreToCategory(c: CoreCategory): DictionaryCategory {
  return { id: c.id, name: c.name, scope: c.scope, entryCount: c.entry_count }
}

function coreToEntry(e: CoreEntry): DictionaryEntry {
  return { id: e.id, categoryId: e.category_id, term: e.term, pinyin: e.pinyin, aliases: e.aliases, note: e.note, enabled: e.enabled }
}

function coreToStats(s: CoreStats): StatsSnapshot {
  return {
    hourlySlots: s.hourly_slots.map((h) => ({ hour: h.hour, minutes: h.minutes, sessionId: h.session_id, title: h.title })),
    weekSessions: s.week_sessions,
    weekMinutes: s.week_minutes,
    monthSessions: s.month_sessions,
    monthMinutes: s.month_minutes,
    totalSessions: s.total_sessions,
    totalMinutes: s.total_minutes,
  }
}

function coreToNote(n: CoreNote): CaptureNote {
  return { id: n.id, content: n.content, timestampMs: n.timestamp_ms, tag: n.tag, segmentId: n.segment_id, createdAt: n.created_at }
}

function coreToVoiceprint(v: CoreVoiceprint) {
  return { id: v.id, name: v.name, embeddingPath: v.embedding_path, dictionaryEntryId: v.dictionary_entry_id, sampleCount: v.sample_count, updatedAt: v.updated_at }
}

function coreSourceLabel(source: CoreTranscriptSegment['source']) {
  if (source === 'system_audio') return '系统音频'
  if (source === 'imported') return '导入音频'
  return '麦克风'
}

function formatStartedAt(value: string) {
  return value.replace('T', ' ').replace(/\.\d+Z$/, 'Z')
}

function formatDuration(startedAt: string, endedAt: string | null) {
  if (!endedAt) return '处理中'
  const started = Date.parse(startedAt)
  const ended = Date.parse(endedAt)
  if (Number.isNaN(started) || Number.isNaN(ended) || ended <= started) return '00:00'
  const totalSeconds = Math.round((ended - started) / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes > 0) return `${minutes} 分 ${String(seconds).padStart(2, '0')} 秒`
  return `00:${String(seconds).padStart(2, '0')}`
}

function pendingRevision(): TranscriptRevision {
  return {
    number: 0,
    provider: '等待转写',
    label: '等待真实转写',
    segments: [],
  }
}

function coreToTimelineRecord(record: CoreTimelineRecord): EvidenceRecord {
  const revisions: TranscriptRevision[] = record.revisions.map((revision) => ({
    number: revision.number,
    provider: revision.provider,
    label: `${revision.provider} · r${revision.number}`,
    createdAt: revision.created_at,
    segments: revision.segments.map((segment): TranscriptSegment => ({
      id: segment.id,
      startMs: segment.start_ms,
      endMs: segment.end_ms,
      source: coreSourceLabel(segment.source),
      text: segment.text,
      chunkId: segment.chunk_id ?? null,
      chunkStartMs: segment.chunk_start_ms ?? null,
      chunkEndMs: segment.chunk_end_ms ?? null,
    })),
  }))
  const latestRevision = revisions.at(-1) ?? pendingRevision()
  const originalRevision = revisions[0] ?? pendingRevision()
  const latestJob = record.latest_job
    ? {
        id: record.latest_job.id,
        state: record.latest_job.state,
        errorCode: record.latest_job.error_code,
        errorSummary: record.latest_job.error_summary,
        chunkId: record.latest_job.chunk_id,
      }
    : null
  const chunks: EvidenceRecord['chunks'] = record.chunks.map((chunk) => ({
    id: chunk.id,
    source: coreSourceLabel(chunk.source),
    audioPath: chunk.audio_path,
    integrityState: chunk.integrity_state,
    errorCode: chunk.error_code,
  }))
  const status: EvidenceRecord['status'] =
    latestJob && ['queued', 'preparing', 'transcribing'].includes(latestJob.state)
      ? 'processing'
      : 'available'

  return {
    id: record.session.id,
    title: record.session.title,
    startedAt: formatStartedAt(record.session.started_at),
    duration: formatDuration(record.session.started_at, record.session.ended_at),
    status,
    chunks,
    latestJob,
    revision: latestRevision,
    originalRevision,
    revisions,
    notes: record.notes.map(coreToNote),
  }
}

function toCoreSource(source: string): CoreTranscriptSegment['source'] {
  if (source === '系统音频') return 'system_audio'
  if (source === '导入音频') return 'imported'
  return 'microphone'
}

// ── Timeline ────────────────────────────────────────────────────────────

export async function loadTimelineRecords() {
  if (!isTauriRuntime()) return demoRecords
  const records = await listTimelineRecords()
  return records.map(coreToTimelineRecord)
}

export async function importAudioRecord(path: string, title: string) {
  if (!isTauriRuntime()) return null
  return importAudioRecordCommand(path, title)
}

export async function appendManualRevision(
  sessionId: string,
  sourceSegments: Array<{ id: string; startMs: number; endMs: number; source: string; text: string }>,
  draft: string,
) {
  if (!isTauriRuntime()) return
  const firstSegment = sourceSegments[0]
  if (!firstSegment) return
  await createManualRevision(sessionId, [
    {
      id: firstSegment.id,
      start_ms: firstSegment.startMs,
      end_ms: firstSegment.endMs,
      source: toCoreSource(firstSegment.source),
      text: draft,
    },
    ...sourceSegments.slice(1).map((segment) => ({
      id: segment.id,
      start_ms: segment.startMs,
      end_ms: segment.endMs,
      source: toCoreSource(segment.source),
      text: segment.text,
    })),
  ])
}

// ── Dictionary ───────────────────────────────────────────────────────────

export async function loadCategories(): Promise<DictionaryCategory[]> {
  if (!isTauriRuntime()) return demoCategories
  try {
    const cats = await listCategories()
    return cats.map(coreToCategory)
  } catch {
    return demoCategories
  }
}

export async function loadEntries(categoryId: string): Promise<DictionaryEntry[]> {
  if (!isTauriRuntime()) return demoEntries.filter((e) => e.categoryId === categoryId)
  try {
    const entries = await listEntries(categoryId)
    return entries.map(coreToEntry)
  } catch {
    return demoEntries.filter((e) => e.categoryId === categoryId)
  }
}

export async function createCategoryAdapter(name: string, scope: string): Promise<DictionaryCategory> {
  if (!isTauriRuntime()) {
    const cat = { id: `dcat-${Date.now()}`, name, scope, entryCount: 0 }
    demoCategories.push(cat)
    return cat
  }
  const c = await createCategory(name, scope)
  return coreToCategory(c)
}

export async function deleteCategoryAdapter(categoryId: string): Promise<void> {
  if (!isTauriRuntime()) return
  await deleteCategory(categoryId)
}

export async function createEntryAdapter(categoryId: string, term: string, pinyin: string, aliases: string, note: string): Promise<DictionaryEntry> {
  if (!isTauriRuntime()) {
    const entry = { id: `dent-${Date.now()}`, categoryId, term, pinyin, aliases, note, enabled: true }
    demoEntries.push(entry)
    return entry
  }
  const e = await createEntry(categoryId, term, pinyin, aliases, note)
  return coreToEntry(e)
}

export async function updateEntryAdapter(entryId: string, term: string, pinyin: string, aliases: string, note: string): Promise<void> {
  if (!isTauriRuntime()) return
  await updateEntry(entryId, term, pinyin, aliases, note)
}

export async function toggleEntryAdapter(entryId: string, enabled: boolean): Promise<void> {
  if (!isTauriRuntime()) return
  await toggleEntry(entryId, enabled)
}

export async function deleteEntryAdapter(entryId: string): Promise<void> {
  if (!isTauriRuntime()) return
  await deleteEntry(entryId)
}

// ── Stats ────────────────────────────────────────────────────────────────

export async function loadStats(): Promise<StatsSnapshot> {
  if (!isTauriRuntime()) return demoStats
  const s = await getStatsSnapshot()
  return coreToStats(s)
}

// ── Voiceprints ──────────────────────────────────────────────────────────

export async function loadVoiceprints(): Promise<CoreVoiceprint[]> {
  if (!isTauriRuntime()) return demoVoiceprints.map((v) => ({
    id: v.id, name: v.name, embedding_path: v.embeddingPath,
    dictionary_entry_id: v.dictionaryEntryId, sample_count: v.sampleCount, updated_at: v.updatedAt,
  }))
  return listVoiceprints()
}

export async function renameVoiceprintAdapter(voiceprintId: string, name: string): Promise<void> {
  if (!isTauriRuntime()) {
    const target = demoVoiceprints.find((voiceprint) => voiceprint.id === voiceprintId)
    if (target) target.name = name
    return
  }
  await renameVoiceprint(voiceprintId, name)
}

export async function deleteVoiceprintAdapter(voiceprintId: string): Promise<void> {
  if (!isTauriRuntime()) {
    const index = demoVoiceprints.findIndex((voiceprint) => voiceprint.id === voiceprintId)
    if (index >= 0) demoVoiceprints.splice(index, 1)
    return
  }
  await deleteVoiceprint(voiceprintId)
}

// ── Notes ────────────────────────────────────────────────────────────────

export async function loadNotes(sessionId: string): Promise<CaptureNote[]> {
  if (!isTauriRuntime()) return demoNotes
  try {
    const notes = await listNotes(sessionId)
    return notes.map(coreToNote)
  } catch {
    return demoNotes
  }
}

export async function createNoteAdapter(sessionId: string, content: string, timestampMs: number, tag: string, segmentId: string | null): Promise<CaptureNote> {
  if (!isTauriRuntime()) {
    const note = { id: `note-${Date.now()}`, content, timestampMs, tag, segmentId, createdAt: new Date().toISOString() }
    demoNotes.push(note)
    return note
  }
  const n = await createNote(sessionId, content, timestampMs, tag, segmentId)
  return coreToNote(n)
}

export async function updateNoteAdapter(noteId: string, content: string, tag: string): Promise<void> {
  if (!isTauriRuntime()) return
  await updateNote(noteId, content, tag)
}

export async function deleteNoteAdapter(noteId: string): Promise<void> {
  if (!isTauriRuntime()) return
  await deleteNote(noteId)
}

// ── Settings (client-side only, no conversion needed) ────────────────────

export async function loadAsrConfig(): Promise<CoreAsrConfig> {
  if (!isTauriRuntime()) return {
    provider: 'sense_voice', model_id: 'sense-voice-small-int8-2024-07-17', language: 'zh', auto_transcribe: true, threads: 4,
    vad_enabled: true, vad_min_speech_ms: 300, vad_silence_ms: 800, itn_enabled: true,
  }
  return getAsrConfig()
}

export async function saveAsrConfig(config: CoreAsrConfig): Promise<void> {
  if (!isTauriRuntime()) return
  await setAsrConfig(config)
}

export async function loadRecordingConfig(): Promise<CoreRecordingConfig> {
  if (!isTauriRuntime()) return {
    capture_mode: 'smart', im_detection_enabled: true,
    im_apps: ['wechat', 'dingtalk', 'feishu', 'teams', 'zoom', 'qq'],
    detection_delay_secs: 3, recovery_delay_secs: 5, sample_rate: 16000,
    storage_path: '~/.lifesub/recordings/',
  }
  return getRecordingConfig()
}

export async function saveRecordingConfig(config: CoreRecordingConfig): Promise<void> {
  if (!isTauriRuntime()) return
  await setRecordingConfig(config)
}

export async function loadRuntimeInfo(): Promise<AppRuntimeInfo> {
  if (!isTauriRuntime()) {
    return {
      app_version: 'web-preview',
      tauri_version: 'browser',
      frontend_stack: 'React 19 + TypeScript',
      asr_runtime: 'desktop runtime unavailable',
    }
  }
  return getAppRuntimeInfo()
}

export async function loadModelCatalog(): Promise<CoreAsrModel[]> {
  if (!isTauriRuntime()) {
    return [
      {
        model_id: 'sense-voice-small-int8-2024-07-17',
        display_name: 'SenseVoice Small',
        provider: 'sense_voice',
        manifest_version: '1',
        bundle_identity: 'demo-sense-voice',
        supported_languages: ['auto', 'zh', 'en', 'ja', 'ko', 'yue'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 1,
        total_bytes: 240500355,
        license_spdx: 'MIT',
        installation_state: 'not_installed',
        selectable: true,
        installable: false,
        executable: false,
        reason_code: 'desktop_runtime_required',
        last_error_code: null,
        download: null,
      },
    ]
  }
  return listAsrModels()
}
