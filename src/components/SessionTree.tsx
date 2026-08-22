import { useState } from 'react'
import { ChevronRight, ChevronDown, Mic, Volume2, FileText, Clock } from 'lucide-react'
import type { EvidenceRecord, TranscriptSegment } from '../domain'

interface SessionTreeProps {
  records: EvidenceRecord[]
  selectedId: string
  onSelect: (id: string) => void
  query: string
}

function formatTime(ms: number) {
  const s = Math.floor(ms / 1000)
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
}

function SegmentIcon({ source }: { source: TranscriptSegment['source'] }) {
  if (source === '系统音频') return <Volume2 size={12} />
  return <Mic size={12} />
}

export function SessionTree({ records, selectedId, onSelect, query }: SessionTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set(records.map((r) => r.id)))

  const statusLabel = (record: EvidenceRecord) => {
    if (record.latestJob?.state === 'blocked_model') return `模型阻塞${record.latestJob.errorCode ? ` · ${record.latestJob.errorCode}` : ''}`
    if (record.latestJob?.state === 'failed') return `转写失败${record.latestJob.errorCode ? ` · ${record.latestJob.errorCode}` : ''}`
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

  const filteredRecords = query
    ? records.filter((r) =>
        r.revision.segments.some((s) => s.text.toLowerCase().includes(query.toLowerCase()))
      )
    : records

  return (
    <aside className="session-tree">
      {filteredRecords.map((record) => {
        const isOpen = expanded.has(record.id)
        const segments = query
          ? record.revision.segments.filter((s) => s.text.toLowerCase().includes(query.toLowerCase()))
          : record.revision.segments

        return (
          <div key={record.id} className="session-tree__group">
            <button
              className={`session-tree__session ${record.id === selectedId ? 'session-tree__session--active' : ''}`}
              onClick={() => { toggle(record.id); onSelect(record.id) }}
            >
              {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <FileText size={14} />
              <span className="session-tree__title">{record.title}</span>
              <span className={`record-status record-status--${record.status}`} />
              <span>{statusLabel(record)}</span>
            </button>
            {isOpen && segments.map((seg) => (
              <button
                key={seg.id}
                className={`session-tree__segment`}
                onClick={() => onSelect(record.id)}
              >
                <SegmentIcon source={seg.source} />
                <span className="session-tree__time">{formatTime(seg.startMs)}</span>
                <span className="session-tree__preview">{seg.text.slice(0, 30)}{seg.text.length > 30 ? '…' : ''}</span>
              </button>
            ))}
            {isOpen && record.notes?.map((note) => (
              <div key={note.id} className="session-tree__note">
                <span className="session-tree__note-tag">{note.tag}</span>
                <span className="session-tree__note-preview">{note.content.slice(0, 25)}…</span>
              </div>
            ))}
            {isOpen && record.status === 'processing' && segments.length === 0 && (
              <div className="session-tree__processing">
                <Clock size={12} /> 处理中...
              </div>
            )}
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
