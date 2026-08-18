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
}

export interface EvidenceRecord {
  id: string
  title: string
  startedAt: string
  duration: string
  status: 'available' | 'processing'
  revision: TranscriptRevision
  originalRevision: TranscriptRevision
  /** 录音过程中添加的笔记 */
  notes: CaptureNote[]
}

// ── New types for UI redesign ──────────────────────────────────────────────

/** 笔记标签 */
export type NoteTag = '待办' | '备忘' | '问题' | '决定' | string

/** 录音过程中添加的笔记 */
export interface CaptureNote {
  id: string
  content: string
  timestampMs: number
  tag: NoteTag
  segmentId: string | null
  createdAt: string
}

/** 捕获模式 */
export type CaptureMode = 'smart' | 'mic-only' | 'system-only'

/** 声纹库中的注册说话人 */
export interface Voiceprint {
  id: string
  name: string
  embeddingPath: string
  dictionaryEntryId: string | null
  sampleCount: number
  updatedAt: string
}

/** 说话人标识 */
export interface Speaker {
  id: string
  label: string
  source: 'voiceprint' | 'dictionary' | 'manual' | 'unknown'
  voiceprintId: string | null
}

/** 实时流式段落 */
export interface LiveSegment {
  id: string
  startMs: number
  speaker: Speaker
  text: string
  completed: boolean
}

/** 词典分类 */
export interface DictionaryCategory {
  id: string
  name: string
  scope: 'global' | string
  entryCount: number
}

/** 词典词条 */
export interface DictionaryEntry {
  id: string
  categoryId: string
  term: string
  pinyin: string
  aliases: string
  note: string
  enabled: boolean
}

/** 24 小时录音统计快照 */
export interface StatsSnapshot {
  hourlySlots: Array<{
    hour: number
    minutes: number
    sessionId: string | null
    title: string | null
  }>
  weekSessions: number
  weekMinutes: number
  monthSessions: number
  monthMinutes: number
  totalSessions: number
  totalMinutes: number
}