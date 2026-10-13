import { useRef, useState } from 'react'
import { ChevronRight, ChevronDown, Mic, Volume2, FileText, Clock } from 'lucide-react'
import type { EvidenceRecord, TranscriptSegment } from '../domain'
import { formatTimelineTime, getRecordMatches, type SegmentSelectionRequest } from './timelineSearch'

interface SessionTreeProps {
  records: EvidenceRecord[]
  selectedId: string
  onSelect: (id: string) => void
  onSelectSegment?: (request: SegmentSelectionRequest) => void
  query: string
}

function SegmentIcon({ source }: { source: TranscriptSegment['source'] }) {
  if (source === '系统音频') return <Volume2 size={12} aria-hidden="true" />
  return <Mic size={12} aria-hidden="true" />
}

export function SessionTree({ records, selectedId, onSelect, onSelectSegment, query }: SessionTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set(records.map((r) => r.id)))
  const segmentRequestId = useRef(0)

  const statusLabel = (record: EvidenceRecord) => {
    if (record.latestJob?.state === 'blocked_model') return `模型阻塞${record.latestJob.errorCode ? ` · ${record.latestJob.errorCode}` : ''}`
    if (record.latestJob?.state === 'failed') return `转写失败${record.latestJob.errorCode ? ` · ${record.latestJob.errorCode}` : ''}`
    if (record.latestJob?.state === 'queued') return '排队等待转写'
    if (record.latestJob?.state === 'preparing') return '正在准备转写'
    if (record.latestJob?.state === 'transcribing') return '正在转写'
    if (record.status === 'processing') return '等待转写'
    if (record.chunks.some((chunk) => chunk.integrityState !== 'available')) return '音频缺失或损坏'
    if (record.chunks.length === 0) return '仅演示'
    return '可播放'
  }

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const setOpen = (id: string, open: boolean) => {
    setExpanded((previous) => {
      const isOpen = previous.has(id)
      if (isOpen === open) return previous
      const next = new Set(previous)
      if (open) next.add(id)
      else next.delete(id)
      return next
    })
  }

  const handleSelectionKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, id: string) => {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return
    event.preventDefault()
    if (query.trim()) return
    setOpen(id, event.key === 'ArrowRight')
  }

  const selectSegment = (recordId: string, segmentId: string) => {
    onSelect(recordId)
    segmentRequestId.current += 1
    onSelectSegment?.({ recordId, segmentId, requestId: segmentRequestId.current })
  }

  const hasActiveQuery = query.trim().length > 0
  const filteredRecords = hasActiveQuery
    ? records.filter((record) => getRecordMatches(record, query).length > 0)
    : records

  return (
    <aside className="session-tree">
      {filteredRecords.map((record) => {
        const segments = hasActiveQuery
          ? getRecordMatches(record, query)
          : record.revision.segments
        const isOpen = hasActiveQuery || expanded.has(record.id)
        const contentsId = `session-${record.id.replace(/[^a-zA-Z0-9_-]/g, '-')}-contents`

        return (
          <div key={record.id} className="session-tree__group">
            <div className={`session-tree__session ${record.id === selectedId ? 'session-tree__session--active' : ''}`}>
              <button
                className="session-tree__toggle"
                aria-label={`${isOpen ? '收起' : '展开'}${record.title}`}
                aria-expanded={isOpen}
                aria-controls={contentsId}
                disabled={hasActiveQuery}
                onClick={() => toggle(record.id)}
              >
                {isOpen ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
              </button>
              <button
                className="session-tree__select"
                aria-label={`查看${record.title}`}
                aria-current={record.id === selectedId ? 'true' : undefined}
                onClick={() => onSelect(record.id)}
                onKeyDown={(event) => handleSelectionKeyDown(event, record.id)}
              >
                <FileText size={14} aria-hidden="true" />
                <span className="session-tree__title">{record.title}</span>
                <span className={`record-status record-status--${record.status}`} aria-hidden="true" />
                <span>{statusLabel(record)}</span>
              </button>
            </div>
            <div id={contentsId} className="session-tree__contents" hidden={!isOpen}>
              {isOpen && (
                <>
                  {segments.map((seg) => (
                    <button key={seg.id} className="session-tree__segment" onClick={() => selectSegment(record.id, seg.id)}>
                      <SegmentIcon source={seg.source} />
                      <span className="session-tree__time">{formatTimelineTime(seg.startMs)}</span>
                      <span className="session-tree__preview">{seg.text.slice(0, 30)}{seg.text.length > 30 ? '…' : ''}</span>
                    </button>
                  ))}
                  {record.notes?.map((note) => (
                    <div key={note.id} className="session-tree__note">
                      <span className="session-tree__note-tag">{note.tag}</span>
                      <span className="session-tree__note-preview">{note.content.slice(0, 25)}…</span>
                    </div>
                  ))}
                  {record.status === 'processing' && segments.length === 0 && (
                    <div className="session-tree__processing">
                      <Clock size={12} aria-hidden="true" /> 处理中...
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        )
      })}
      {filteredRecords.length === 0 && (
        <div className="empty-state">
          <strong>没有匹配的会话</strong>
          <p>换一个关键词搜索，或导入新的音频。</p>
        </div>
      )}
    </aside>
  )
}
