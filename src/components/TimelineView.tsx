import { useState, useMemo } from 'react'
import { Search, Upload } from 'lucide-react'
import { SessionTree } from './SessionTree'
import { TranscriptView } from './TranscriptView'
import { StatsBar } from './StatsBar'
import { demoStats } from '../data/demo'
import type { EvidenceRecord, TranscriptRevision } from '../domain'

interface TimelineViewProps {
  records: EvidenceRecord[]
  onRecordsChange: (records: EvidenceRecord[]) => void
  onNotice: (msg: string) => void
}

export function TimelineView({ records, onRecordsChange, onNotice }: TimelineViewProps) {
  const [selectedId, setSelectedId] = useState(records[0]?.id ?? '')
  const [query, setQuery] = useState('')

  const selectedRecord = useMemo(
    () => records.find((r) => r.id === selectedId) ?? records[0],
    [records, selectedId]
  )

  const handleRevisionChange = (revision: TranscriptRevision) => {
    onRecordsChange(
      records.map((r) =>
        r.id === selectedId ? { ...r, revision } : r
      )
    )
  }

  return (
    <main className="timeline-view">
      <header className="timeline-view__toolbar">
        <div className="search-field">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            aria-label="搜索转写"
            placeholder="搜索原话、来源或时间…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button className="button" onClick={() => onNotice('导入音频功能：选择本地音频文件。')}>
          <Upload size={16} />导入音频
        </button>
      </header>

      <div className="timeline-view__content">
        <SessionTree
          records={records}
          selectedId={selectedId}
          onSelect={setSelectedId}
          query={query}
        />
        {selectedRecord && (
          <TranscriptView
            record={selectedRecord}
            query={query}
            onRevisionChange={handleRevisionChange}
            onNotice={onNotice}
          />
        )}
      </div>

      <StatsBar stats={demoStats} />
    </main>
  )
}