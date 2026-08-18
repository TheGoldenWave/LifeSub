import { FileAudio, Search } from 'lucide-react'
import type { EvidenceRecord } from '../domain'

interface RecordListProps {
  records: EvidenceRecord[]
  selectedId: string
  query: string
  onQueryChange: (query: string) => void
  onSelect: (id: string) => void
}

export function RecordList({ records, selectedId, query, onQueryChange, onSelect }: RecordListProps) {
  return (
    <aside className="record-list">
      <div className="search-field">
        <Search size={16} aria-hidden="true" />
        <input type="search" aria-label="搜索转写" placeholder="搜索原话、来源或时间…" value={query} onChange={(event) => onQueryChange(event.target.value)} />
      </div>
      <div className="record-list__heading"><span>最近记录</span><small>{records.length} 条</small></div>
      <div className="record-list__items">
        {records.map((record) => (
          <button key={record.id} className={`record-row ${record.id === selectedId ? 'record-row--active' : ''}`} onClick={() => onSelect(record.id)}>
            <FileAudio size={17} aria-hidden="true" />
            <span><strong>{record.title}</strong><small>{record.startedAt} · {record.duration}</small></span>
            <i className={`record-status record-status--${record.status}`} aria-label={record.status === 'available' ? '证据可用' : '正在处理'} />
          </button>
        ))}
      </div>
    </aside>
  )
}
