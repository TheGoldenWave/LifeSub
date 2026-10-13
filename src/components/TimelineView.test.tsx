import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { TimelineView } from './TimelineView'
import type { EvidenceRecord } from '../domain'

vi.mock('../services/markdown', () => ({ exportRecordMarkdown: vi.fn() }))

const records: EvidenceRecord[] = [
  {
    id: 'rec_1', title: '晨会', startedAt: '2026-09-03 09:30', duration: '10:00', status: 'available', chunks: [], latestJob: null,
    originalRevision: { number: 1, provider: 'sense_voice', label: '原始转写 · r1', segments: [
      { id: 'seg_1', startMs: 65_000, endMs: 70_000, source: '麦克风', text: '确认发布范围' },
      { id: 'seg_2', startMs: 125_000, endMs: 130_000, source: '系统音频', text: '下一轮安排' },
    ] },
    revision: { number: 1, provider: 'sense_voice', label: '原始转写 · r1', segments: [
      { id: 'seg_1', startMs: 65_000, endMs: 70_000, source: '麦克风', text: '确认发布范围' },
      { id: 'seg_2', startMs: 125_000, endMs: 130_000, source: '系统音频', text: '下一轮安排' },
    ] },
    revisions: [], notes: [],
  },
  {
    id: 'rec_2', title: '访谈', startedAt: '2026-09-03 14:00', duration: '20:00', status: 'available', chunks: [], latestJob: null,
    originalRevision: { number: 1, provider: 'sense_voice', label: '原始转写 · r1', segments: [
      { id: 'seg_3', startMs: 65_000, endMs: 72_000, source: '麦克风', text: '用户反馈' },
    ] },
    revision: { number: 1, provider: 'sense_voice', label: '原始转写 · r1', segments: [
      { id: 'seg_3', startMs: 65_000, endMs: 72_000, source: '麦克风', text: '用户反馈' },
    ] },
    revisions: [], notes: [],
  },
]

const renderTimeline = (nextRecords = records) => render(
  <TimelineView
    records={nextRecords}
    onRecordsChange={vi.fn()}
    onNotice={vi.fn()}
    onImportAudio={vi.fn()}
  />
)

describe('TimelineView', () => {
  it('searches body, source, and formatted time with counts and clear action', async () => {
    const user = userEvent.setup()
    renderTimeline()

    const search = screen.getByRole('searchbox', { name: '搜索转写' })
    await user.type(search, '01:05')
    expect(screen.getByText('2 个会话 · 2 处命中')).toBeInTheDocument()
    expect(screen.getAllByText('确认发布范围')).not.toHaveLength(0)
    expect(screen.getAllByText('用户反馈')).not.toHaveLength(0)

    await user.clear(search)
    await user.type(search, '系统音频')
    expect(screen.getByText('1 个会话 · 1 处命中')).toBeInTheDocument()
    expect(screen.getAllByText('下一轮安排')).not.toHaveLength(0)

    await user.click(screen.getByRole('button', { name: '清除搜索' }))
    expect(search).toHaveValue('')
    expect(screen.queryByText(/处命中/)).not.toBeInTheDocument()
  })

  it('keeps the query when returning from detail and falls back to list when records clear', async () => {
    const user = userEvent.setup()
    const { container, rerender } = renderTimeline()
    const search = screen.getByRole('searchbox', { name: '搜索转写' })
    await user.type(search, '发布')

    await user.click(screen.getByRole('button', { name: '查看晨会' }))
    expect(container.querySelector('.timeline-view')).toHaveClass('timeline-view--detail')
    await user.click(screen.getByRole('button', { name: '返回会话列表' }))
    expect(container.querySelector('.timeline-view')).toHaveClass('timeline-view--list')
    expect(search).toHaveValue('发布')

    await user.click(screen.getByRole('button', { name: '查看晨会' }))
    rerender(
      <TimelineView records={[]} onRecordsChange={vi.fn()} onNotice={vi.fn()} onImportAudio={vi.fn()} />
    )
    expect(container.querySelector('.timeline-view')).toHaveClass('timeline-view--list')
  })

  it('forwards repeated segment selections to the transcript detail', async () => {
    const user = userEvent.setup()
    const scrollIntoView = vi.fn()
    Element.prototype.scrollIntoView = scrollIntoView
    renderTimeline()

    const segment = screen.getByRole('button', { name: '01:05 确认发布范围' })
    await user.click(segment)
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledTimes(1))
    await user.click(segment)
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledTimes(2))
  })
})
