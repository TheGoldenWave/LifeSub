import type { DictionaryCategory, DictionaryEntry, StatsSnapshot, CaptureNote } from '../domain'
import {
  isTauriRuntime,
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
} from '../services/lifesub'
import {
  demoCategories,
  demoEntries,
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
  try {
    const s = await getStatsSnapshot()
    return coreToStats(s)
  } catch {
    return demoStats
  }
}

// ── Voiceprints ──────────────────────────────────────────────────────────

export async function loadVoiceprints(): Promise<CoreVoiceprint[]> {
  if (!isTauriRuntime()) return demoVoiceprints.map((v) => ({
    id: v.id, name: v.name, embedding_path: v.embeddingPath,
    dictionary_entry_id: v.dictionaryEntryId, sample_count: v.sampleCount, updated_at: v.updatedAt,
  }))
  try {
    return await listVoiceprints()
  } catch {
    return demoVoiceprints.map((v) => ({
      id: v.id, name: v.name, embedding_path: v.embeddingPath,
      dictionary_entry_id: v.dictionaryEntryId, sample_count: v.sampleCount, updated_at: v.updatedAt,
    }))
  }
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
    provider: 'sense_voice', language: 'zh', auto_transcribe: true, threads: 4,
    vad_enabled: true, vad_min_speech_ms: 300, vad_silence_ms: 800, itn_enabled: true,
  }
  try {
    return await getAsrConfig()
  } catch {
    return { provider: 'sense_voice', language: 'zh', auto_transcribe: true, threads: 4, vad_enabled: true, vad_min_speech_ms: 300, vad_silence_ms: 800, itn_enabled: true }
  }
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
  try {
    return await getRecordingConfig()
  } catch {
    return { capture_mode: 'smart', im_detection_enabled: true, im_apps: ['wechat', 'dingtalk', 'feishu', 'teams', 'zoom', 'qq'], detection_delay_secs: 3, recovery_delay_secs: 5, sample_rate: 16000, storage_path: '~/.lifesub/recordings/' }
  }
}

export async function saveRecordingConfig(config: CoreRecordingConfig): Promise<void> {
  if (!isTauriRuntime()) return
  await setRecordingConfig(config)
}