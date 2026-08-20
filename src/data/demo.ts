import type { EvidenceRecord } from '../domain'

export const demoRecords: EvidenceRecord[] = [
  {
    id: 'rec_001',
    title: '产品评审',
    startedAt: '2026-08-15 14:30',
    duration: '03:42',
    status: 'available',
    originalRevision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [
        { id: 'seg_001', startMs: 0, endMs: 42000, source: '系统音频', text: '证据链必须保留原始转写，修订不能覆盖首版。' },
        { id: 'seg_002', startMs: 45000, endMs: 98000, source: '麦克风', text: '首版重点是可靠、可定位的声音证据。' },
        { id: 'seg_003', startMs: 102000, endMs: 162000, source: '系统音频', text: '每个 revision 都能追溯到 Provider、模型和时间。' },
      ],
    },
    revision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [
        { id: 'seg_001', startMs: 0, endMs: 42000, source: '系统音频', text: '证据链必须保留原始转写，修订不能覆盖首版。' },
        { id: 'seg_002', startMs: 45000, endMs: 98000, source: '麦克风', text: '首版重点是可靠、可定位的声音证据。' },
        { id: 'seg_003', startMs: 102000, endMs: 162000, source: '系统音频', text: '每个 revision 都能追溯到 Provider、模型和时间。' },
      ],
    },
  },
  {
    id: 'rec_002',
    title: '架构讨论',
    startedAt: '2026-08-15 10:15',
    duration: '12:08',
    status: 'available',
    originalRevision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [
        { id: 'seg_004', startMs: 0, endMs: 280000, source: '系统音频', text: '先确认首版范围，再决定是否扩展云端与校对能力。' },
        { id: 'seg_005', startMs: 290000, endMs: 480000, source: '麦克风', text: '所有本地处理结果必须可独立验证，不依赖外部服务。' },
      ],
    },
    revision: {
      number: 1,
      provider: '本地演示 ASR',
      label: '原始转写 · r1',
      segments: [
        { id: 'seg_004', startMs: 0, endMs: 280000, source: '系统音频', text: '先确认首版范围，再决定是否扩展云端与校对能力。' },
        { id: 'seg_005', startMs: 290000, endMs: 480000, source: '麦克风', text: '所有本地处理结果必须可独立验证，不依赖外部服务。' },
      ],
    },
  },
]