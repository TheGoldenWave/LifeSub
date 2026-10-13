import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { SessionTree } from './SessionTree'
import type { EvidenceRecord } from '../domain'

const records: EvidenceRecord[] = [{
  id: 'rec_1',
  title: '设计评审',
  startedAt: '2026-09-03 09:30',
  duration: '12:00',
  status: 'available',
  chunks: [],
  latestJob: null,
  originalRevision: {
    number: 1,
    provider: 'sense_voice',
    label: '原始转写 · r1',
    segments: [
      { id: 'seg_1', startMs: 65_000, endMs: 70_000, source: '麦克风', text: '讨论移动端布局' },
    ],
  },
  revision: {
    number: 1,
    provider: 'sense_voice',
    label: '原始转写 · r1',
    segments: [
      { id: 'seg_1', startMs: 65_000, endMs: 70_000, source: '麦克风', text: '讨论移动端布局' },
    ],
  },
  revisions: [],
  notes: [],
}]

describe('SessionTree', () => {
  it.each([
    ['queued', '排队等待转写'],
    ['preparing', '正在准备转写'],
    ['transcribing', '正在转写'],
  ])('shows the persisted %s Job stage', (state, label) => {
    render(<SessionTree records={[{
      ...records[0], status: 'processing',
      latestJob: { id: 'job-1', state, chunkId: 'chunk-1' },
    }]} selectedId="rec_1" onSelect={vi.fn()} query="" />)
    expect(screen.getByRole('button', { name: '查看设计评审' })).toHaveTextContent(label)
  })

  it('separates session selection from expansion controls', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()

    render(<SessionTree records={records} selectedId="" onSelect={onSelect} query="" />)

    const toggle = screen.getByRole('button', { name: '收起设计评审' })
    const select = screen.getByRole('button', { name: '查看设计评审' })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    expect(toggle).toHaveAttribute('aria-controls', 'session-rec_1-contents')

    await user.click(select)
    expect(onSelect).toHaveBeenCalledWith('rec_1')
    expect(toggle).toHaveAttribute('aria-expanded', 'true')

    await user.click(toggle)
    expect(onSelect).toHaveBeenCalledTimes(1)
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByText('讨论移动端布局')).not.toBeInTheDocument()
  })

  it('forces matching context open while a query is active', async () => {
    const user = userEvent.setup()
    const { rerender } = render(<SessionTree records={records} selectedId="" onSelect={vi.fn()} query="" />)

    await user.click(screen.getByRole('button', { name: '收起设计评审' }))
    rerender(<SessionTree records={records} selectedId="" onSelect={vi.fn()} query="麦克风" />)

    expect(screen.getByRole('button', { name: '收起设计评审' })).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('讨论移动端布局')).toBeInTheDocument()
  })

  it('supports ArrowRight expansion and ArrowLeft collapse on the selection control', async () => {
    const user = userEvent.setup()
    const { rerender } = render(<SessionTree records={records} selectedId="rec_1" onSelect={vi.fn()} query="" />)
    const select = screen.getByRole('button', { name: '查看设计评审' })

    select.focus()
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByRole('button', { name: '展开设计评审' })).toHaveAttribute('aria-expanded', 'false')

    select.focus()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('button', { name: '收起设计评审' })).toHaveAttribute('aria-expanded', 'true')

    await user.click(screen.getByRole('button', { name: '收起设计评审' }))
    rerender(<SessionTree records={records} selectedId="rec_1" onSelect={vi.fn()} query="麦克风" />)
    screen.getByRole('button', { name: '查看设计评审' }).focus()
    await user.keyboard('{ArrowLeft}')
    rerender(<SessionTree records={records} selectedId="rec_1" onSelect={vi.fn()} query="" />)
    expect(screen.getByRole('button', { name: '展开设计评审' })).toHaveAttribute('aria-expanded', 'false')
  })

  it('emits a fresh request when the same segment is selected repeatedly', async () => {
    const user = userEvent.setup()
    const onSelectSegment = vi.fn()

    render(
      <SessionTree
        records={records}
        selectedId="rec_1"
        onSelect={vi.fn()}
        onSelectSegment={onSelectSegment}
        query=""
      />
    )

    const segment = screen.getByRole('button', { name: '01:05 讨论移动端布局' })
    await user.click(segment)
    await user.click(segment)

    expect(onSelectSegment).toHaveBeenNthCalledWith(1, { recordId: 'rec_1', segmentId: 'seg_1', requestId: 1 })
    expect(onSelectSegment).toHaveBeenNthCalledWith(2, { recordId: 'rec_1', segmentId: 'seg_1', requestId: 2 })
  })
})
