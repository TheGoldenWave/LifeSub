import type { EvidenceRecord, CaptureNote, DictionaryCategory, DictionaryEntry, Voiceprint, StatsSnapshot } from '../domain'

const originalSegments = [
  { id: 'seg-001', startMs: 12_000, endMs: 19_500, source: '麦克风' as const, text: '我们今天先确认首版范围，重点是把基础闭环真正跑起来。' },
  { id: 'seg-002', startMs: 24_300, endMs: 31_800, source: '系统音频' as const, text: '证据链必须保留原始转写，并且每次修改都能追溯。' },
  { id: 'seg-003', startMs: 42_000, endMs: 49_600, source: '麦克风' as const, text: '搜索结果要能回到准确的音频时间范围。' },
]

export const demoNotes: CaptureNote[] = [
  {
    id: 'note-001',
    content: '确认 ASR Provider 切换时机',
    timestampMs: 4_000,
    tag: '待办',
    segmentId: 'seg-001',
    createdAt: '2026-08-18T16:20:00Z',
  },
  {
    id: 'note-002',
    content: '证据链包含原始音频+转写+修订记录',
    timestampMs: 20_000,
    tag: '备忘',
    segmentId: 'seg-002',
    createdAt: '2026-08-18T16:22:00Z',
  },
  {
    id: 'note-003',
    content: '落实搜索结果时间戳关联',
    timestampMs: 32_000,
    tag: '待办',
    segmentId: 'seg-003',
    createdAt: '2026-08-18T16:24:00Z',
  },
]

export const demoRecords: EvidenceRecord[] = [
  {
    id: 'rec-20260815-001',
    title: 'LifeSub 首版范围讨论',
    startedAt: '今天 16:18',
    duration: '38 分钟',
    status: 'available',
    originalRevision: { number: 1, provider: '本地演示 ASR', label: '原始转写 · r1', segments: originalSegments },
    revision: { number: 1, provider: '本地演示 ASR', label: '原始转写 · r1', segments: originalSegments },
    notes: demoNotes,
  },
  {
    id: 'rec-20260814-002',
    title: '架构边界复盘',
    startedAt: '昨天 10:05',
    duration: '22 分钟',
    status: 'available',
    originalRevision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [{ id: 'seg-004', startMs: 7_500, endMs: 16_200, source: '麦克风', text: 'LifeSub 只保存发生过什么，上层系统负责解释。' }],
    },
    revision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [{ id: 'seg-004', startMs: 7_500, endMs: 16_200, source: '麦克风', text: 'LifeSub 只保存发生过什么，上层系统负责解释。' }],
    },
    notes: [],
  },
]

export const demoCategories: DictionaryCategory[] = [
  { id: 'cat-1', name: '人名', scope: 'global', entryCount: 8 },
  { id: 'cat-2', name: '地名', scope: 'global', entryCount: 12 },
  { id: 'cat-3', name: '专业术语', scope: 'global', entryCount: 25 },
  { id: 'cat-4', name: '项目名', scope: 'global', entryCount: 6 },
  { id: 'cat-5', name: '品牌名', scope: 'global', entryCount: 3 },
  { id: 'cat-proj-1', name: 'LifeSub 术语', scope: 'project:lifesub', entryCount: 4 },
]

export const demoEntries: DictionaryEntry[] = [
  { id: 'ent-1', categoryId: 'cat-1', term: '张伟', pinyin: 'zhāng wěi', aliases: '张总;伟哥', note: '产品负责人', enabled: true },
  { id: 'ent-2', categoryId: 'cat-1', term: '李娜', pinyin: 'lǐ nà', aliases: '', note: '', enabled: true },
  { id: 'ent-3', categoryId: 'cat-1', term: '刘洋', pinyin: 'liú yáng', aliases: '', note: '', enabled: false },
]

export const demoVoiceprints: Voiceprint[] = [
  { id: 'vp-1', name: '张伟', embeddingPath: '~/.lifesub/voiceprints/vp-1.bin', dictionaryEntryId: 'ent-1', sampleCount: 12, updatedAt: '2026-08-18T16:00:00Z' },
  { id: 'vp-2', name: '我', embeddingPath: '~/.lifesub/voiceprints/vp-2.bin', dictionaryEntryId: null, sampleCount: 8, updatedAt: '2026-08-17T10:00:00Z' },
]

export const demoStats: StatsSnapshot = {
  hourlySlots: Array.from({ length: 24 }, (_, i) => ({
    hour: i,
    minutes: i === 15 ? 22 : i === 16 ? 38 : 0,
    sessionId: i === 15 ? 'rec-20260814-002' : i === 16 ? 'rec-20260815-001' : null,
    title: i === 15 ? '架构边界复盘' : i === 16 ? 'LifeSub 首版范围讨论' : null,
  })),
  weekSessions: 3,
  weekMinutes: 62,
  monthSessions: 8,
  monthMinutes: 180,
  totalSessions: 42,
  totalMinutes: 1560,
}