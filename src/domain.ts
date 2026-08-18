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
}
