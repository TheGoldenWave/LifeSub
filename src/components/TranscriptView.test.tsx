import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { TranscriptView } from './TranscriptView'
import type { EvidenceRecord } from '../domain'

const baseRecord: EvidenceRecord = {
  id: 'rec_1',
  title: '真实导入样本',
  startedAt: '2026-08-19 16:00',
  duration: '00:30',
  status: 'available',
  chunks: [{
    id: 'chk_1',
    source: '导入音频',
    audioPath: '/tmp/lifesub/audio.wav',
    integrityState: 'available',
    errorCode: null,
  }, {
    id: 'chk_2',
    source: '导入音频',
    audioPath: '/tmp/lifesub/audio-2.wav',
    integrityState: 'available',
    errorCode: null,
  }],
  latestJob: null,
  originalRevision: {
    number: 1,
    provider: 'sense_voice',
    label: '原始转写 · r1',
    segments: [{ id: 'seg_1', startMs: 0, endMs: 30_000, source: '导入音频', text: '原始转写', chunkId: 'chk_1', chunkStartMs: 0, chunkEndMs: 30_000 }],
  },
  revision: {
    number: 2,
    provider: '人工修订',
    label: '人工修订 · r2',
    segments: [{ id: 'seg_1', startMs: 0, endMs: 30_000, source: '导入音频', text: '人工修订后的文本', chunkId: 'chk_1', chunkStartMs: 0, chunkEndMs: 30_000 }],
  },
  revisions: [
    {
      number: 1,
      provider: 'sense_voice',
      label: '原始转写 · r1',
      segments: [{ id: 'seg_1', startMs: 0, endMs: 30_000, source: '导入音频', text: '原始转写', chunkId: 'chk_1', chunkStartMs: 0, chunkEndMs: 30_000 }],
    },
    {
      number: 2,
      provider: '人工修订',
      label: '人工修订 · r2',
      segments: [
        { id: 'seg_1', startMs: 0, endMs: 30_000, source: '导入音频', text: '人工修订后的文本', chunkId: 'chk_1', chunkStartMs: 0, chunkEndMs: 30_000 },
        { id: 'seg_2', startMs: 31_000, endMs: 45_000, source: '导入音频', text: '第二段', chunkId: 'chk_2', chunkStartMs: 1_000, chunkEndMs: 15_000 },
      ],
    },
  ],
  notes: [],
}

describe('TranscriptView', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('shows a missing-audio error when playback is unavailable', async () => {
    const user = userEvent.setup()
    const onNotice = vi.fn()

    render(
      <TranscriptView
        record={{ ...baseRecord, chunks: [] }}
        query=""
        onRevisionChange={vi.fn()}
        onNotice={onNotice}
      />
    )

    await user.click(screen.getByRole('button', { name: '播放 00:00' }))

    expect(onNotice).toHaveBeenCalledWith('找不到这条记录对应的音频文件。')
  })

  it('shows revision history as read-only and updates playback status', async () => {
    const user = userEvent.setup()
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})

    const { container } = render(
      <TranscriptView
        record={baseRecord}
        query=""
        onRevisionChange={vi.fn()}
        onNotice={vi.fn()}
      />
    )

    await user.click(screen.getByRole('button', { name: '查看原始 r1' }))
    expect(screen.getByText('历史 revision 只读')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '创建修订' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '播放 00:00' }))
    expect(play).toHaveBeenCalled()

    const audio = container.querySelector('audio')
    expect(audio).not.toBeNull()
    Object.defineProperty(audio!, 'duration', { value: 30, configurable: true })
    Object.defineProperty(audio!, 'currentTime', { value: 5, configurable: true })
    fireEvent.timeUpdate(audio!)
    expect(screen.getByText('播放中 00:05 / 00:30')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '暂停播放 00:00' }))
    expect(pause).toHaveBeenCalled()
  })

  it('plays the correct chunk for each segment', async () => {
    const user = userEvent.setup()
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)

    const { container } = render(
      <TranscriptView
        record={baseRecord}
        query=""
        onRevisionChange={vi.fn()}
        onNotice={vi.fn()}
      />
    )

    await user.click(screen.getByRole('button', { name: '查看 r2' }))
    await user.click(screen.getByRole('button', { name: '播放 00:31' }))

    expect(play).toHaveBeenCalled()
    const audio = container.querySelector('audio')
    expect(audio?.getAttribute('src')).toContain('audio-2.wav')
  })

  it('refuses playback when an explicit chunk binding is unknown', async () => {
    const user = userEvent.setup()
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    const onNotice = vi.fn()
    const record = {
      ...baseRecord,
      revisions: [],
      revision: {
        ...baseRecord.revision,
        segments: [{
          ...baseRecord.revision.segments[0],
          chunkId: 'chk_missing',
        }],
      },
    }

    render(
      <TranscriptView
        record={record}
        query=""
        onRevisionChange={vi.fn()}
        onNotice={onNotice}
      />
    )

    await user.click(screen.getByRole('button', { name: '播放 00:00' }))

    expect(play).not.toHaveBeenCalled()
    expect(onNotice).toHaveBeenCalledWith('找不到这条记录对应的音频文件。')
  })
})
