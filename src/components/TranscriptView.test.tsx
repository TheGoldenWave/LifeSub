import { fireEvent, render, screen, waitFor } from '@testing-library/react'
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
        onExportMarkdown={vi.fn()}
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
        onExportMarkdown={vi.fn()}
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
        onExportMarkdown={vi.fn()}
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
        onExportMarkdown={vi.fn()}
        onNotice={onNotice}
      />
    )

    await user.click(screen.getByRole('button', { name: '播放 00:00' }))

    expect(play).not.toHaveBeenCalled()
    expect(onNotice).toHaveBeenCalledWith('找不到这条记录对应的音频文件。')
  })

  it('reports Markdown success only after the export is durably written', async () => {
    const user = userEvent.setup()
    const onNotice = vi.fn()
    const onExportMarkdown = vi.fn().mockRejectedValue(new Error('write failed'))

    render(
      <TranscriptView
        record={baseRecord}
        query=""
        onRevisionChange={vi.fn()}
        onExportMarkdown={onExportMarkdown}
        onNotice={onNotice}
      />
    )

    await user.click(screen.getByRole('button', { name: '导出 Markdown' }))

    expect(onExportMarkdown).toHaveBeenCalledWith(baseRecord, 2)
    expect(onNotice).toHaveBeenCalledWith('Markdown 导出失败，请重试。')
    expect(onNotice).not.toHaveBeenCalledWith('Markdown 已按当前 revision 导出。')
  })

  it('moves between transcript matches and reports the current match position', async () => {
    const user = userEvent.setup()

    render(
      <TranscriptView
        record={baseRecord}
        query="导入音频"
        onRevisionChange={vi.fn()}
        onExportMarkdown={vi.fn()}
        onNotice={vi.fn()}
      />
    )

    expect(screen.getByText('1 / 2')).toBeInTheDocument()
    expect(screen.getByText('人工修订后的文本').closest('article')).toHaveClass('segment--search-current')

    await user.click(screen.getByRole('button', { name: '下一处命中' }))
    expect(screen.getByText('2 / 2')).toBeInTheDocument()
    expect(screen.getByText('第二段').closest('article')).toHaveClass('segment--search-current')

    await user.click(screen.getByRole('button', { name: '上一处命中' }))
    expect(screen.getByText('1 / 2')).toBeInTheDocument()
  })

  it('resumes paused audio from the persistent playback controls and clears on revision change', async () => {
    const user = userEvent.setup()
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})

    render(
      <TranscriptView
        record={baseRecord}
        query=""
        onRevisionChange={vi.fn()}
        onExportMarkdown={vi.fn()}
        onNotice={vi.fn()}
      />
    )

    await user.click(screen.getByRole('button', { name: '播放 00:00' }))
    expect(screen.getByRole('region', { name: '音频播放控制' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '暂停音频' }))
    expect(pause).toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '继续播放音频' }))
    expect(play).toHaveBeenCalledTimes(2)

    await user.click(screen.getByRole('button', { name: '查看原始 r1' }))
    expect(screen.queryByRole('region', { name: '音频播放控制' })).not.toBeInTheDocument()
  })

  it('locates and plays repeated segment requests from the session tree', async () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})
    const scrollIntoView = vi.fn()
    Element.prototype.scrollIntoView = scrollIntoView

    const props = {
      record: baseRecord,
      query: '',
      onRevisionChange: vi.fn(),
      onExportMarkdown: vi.fn(),
      onNotice: vi.fn(),
    }
    const { rerender } = render(<TranscriptView {...props} segmentRequest={null} />)

    rerender(<TranscriptView {...props} segmentRequest={{ recordId: 'rec_1', segmentId: 'seg_1', requestId: 1 }} />)
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledTimes(1))
    expect(play).toHaveBeenCalledTimes(1)

    rerender(<TranscriptView {...props} segmentRequest={{ recordId: 'rec_1', segmentId: 'seg_1', requestId: 2 }} />)
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledTimes(2))
    expect(pause).toHaveBeenCalled()
  })

  it('locates unavailable segment requests without showing a playback error', async () => {
    const onNotice = vi.fn()
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    const scrollIntoView = vi.fn()
    Element.prototype.scrollIntoView = scrollIntoView
    const record = { ...baseRecord, chunks: [] }

    render(
      <TranscriptView
        record={record}
        query=""
        segmentRequest={{ recordId: 'rec_1', segmentId: 'seg_1', requestId: 1 }}
        onRevisionChange={vi.fn()}
        onExportMarkdown={vi.fn()}
        onNotice={onNotice}
      />
    )

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled())
    expect(play).not.toHaveBeenCalled()
    expect(onNotice).not.toHaveBeenCalled()
  })
})
