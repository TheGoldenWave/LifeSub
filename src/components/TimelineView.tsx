import { useEffect, useMemo, useState } from 'react'
import { Search, Upload } from 'lucide-react'
import { SessionTree } from './SessionTree'
import { TranscriptView } from './TranscriptView'
import { StatsBar } from './StatsBar'
import { appendManualRevision, loadTimelineRecords } from '../data/adapter'
import type { EvidenceRecord } from '../domain'

interface TimelineViewProps {
  records: EvidenceRecord[]
  onRecordsChange: (records: EvidenceRecord[]) => void
  onNotice: (msg: string) => void
  onImportAudio: () => void | Promise<void>
  loading?: boolean
  error?: string
  onRetry?: () => void | Promise<void>
}

export function TimelineView({ records, onRecordsChange, onNotice, onImportAudio, loading = false, error = '', onRetry }: TimelineViewProps) {
  const [selectedId, setSelectedId] = useState(records[0]?.id ?? '')
  const [query, setQuery] = useState('')

  useEffect(() => {
    if (!records.length) {
      setSelectedId('')
      return
    }
    if (!records.some((record) => record.id === selectedId)) {
      setSelectedId(records[0]?.id ?? '')
    }
  }, [records, selectedId])

  const selectedRecord = useMemo(
    () => records.find((r) => r.id === selectedId) ?? records[0],
    [records, selectedId]
  )

  const handleRevisionChange = async (draft: string) => {
    const record = records.find((candidate) => candidate.id === selectedId)
    if (!record) return
    if (!record.revisions.length) {
      onNotice('当前记录还没有真实转写，暂时无法创建修订。')
      return
    }

    if (record.chunks.length > 0) {
      const latestRevision = record.revisions.at(-1) ?? record.revision
      await appendManualRevision(record.id, latestRevision.segments, draft)
      const nextRecords = await loadTimelineRecords()
      onRecordsChange(nextRecords)
      return
    }

    const latestRevision = record.revisions.at(-1) ?? record.revision
    const nextRevision = {
      number: latestRevision.number + 1,
      provider: '人工修订',
      label: `人工修订 · r${latestRevision.number + 1}`,
      segments: [
        { ...latestRevision.segments[0], text: draft },
        ...latestRevision.segments.slice(1),
      ],
    }
    onRecordsChange(records.map((candidate) => candidate.id === selectedId ? {
      ...candidate,
      revision: nextRevision,
      revisions: [...candidate.revisions, nextRevision],
    } : candidate))
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
        <button className="button" onClick={() => void onImportAudio()}>
          <Upload size={16} />导入音频
        </button>
      </header>

      {loading && <div role="status">正在从 Catalog 加载记录…</div>}
      {!loading && error && (
        <div role="alert">
          时间线加载失败：{error}
          {onRetry && <button className="text-button" onClick={() => void onRetry()}>重试时间线加载</button>}
        </div>
      )}

      {!loading && !error && <div className="timeline-view__content">
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
      </div>}

      <StatsBar />
    </main>
  )
}
