import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, Search, Upload, X } from 'lucide-react'
import { SessionTree } from './SessionTree'
import { TranscriptView } from './TranscriptView'
import { StatsBar } from './StatsBar'
import { appendManualRevision, loadTimelineRecords } from '../data/adapter'
import type { EvidenceRecord } from '../domain'
import { exportRecordMarkdown } from '../services/markdown'
import { getTimelineSearchSummary, type SegmentSelectionRequest } from './timelineSearch'

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
  const [viewMode, setViewMode] = useState<'list' | 'detail'>('list')
  const [segmentRequest, setSegmentRequest] = useState<SegmentSelectionRequest | null>(null)

  useEffect(() => {
    if (!records.length) {
      setSelectedId('')
      setViewMode('list')
      setSegmentRequest(null)
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
  const searchSummary = useMemo(() => getTimelineSearchSummary(records, query), [records, query])

  const selectRecord = (id: string) => {
    setSelectedId(id)
    setViewMode('detail')
    setSegmentRequest(null)
  }

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
    <main className={`timeline-view timeline-view--${viewMode}`}>
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
          {query && (
            <button className="search-field__clear" aria-label="清除搜索" title="清除搜索" onClick={() => setQuery('')}>
              <X size={15} aria-hidden="true" />
            </button>
          )}
        </div>
        {query && (
          <span className="timeline-view__search-summary" role="status" aria-live="polite">
            {searchSummary.sessionCount} 个会话 · {searchSummary.matchCount} 处命中
          </span>
        )}
        <button className="button" title="导入新的音频记录" onClick={() => void onImportAudio()}>
          <Upload size={16} />导入音频
        </button>
      </header>

      {loading && (
        <div className="timeline-view__banner timeline-view__banner--loading" role="status" aria-live="polite">
          正在从 Catalog 加载记录…
        </div>
      )}
      {!loading && error && (
        <div className="timeline-view__banner timeline-view__banner--error" role="alert">
          <span>时间线加载失败：{error}</span>
          {onRetry && <button className="text-button" onClick={() => void onRetry()}>重试时间线加载</button>}
        </div>
      )}

      {!loading && !error && <div className="timeline-view__content">
        <div className="timeline-view__list-pane">
          <SessionTree
            records={records}
            selectedId={selectedId}
            onSelect={selectRecord}
            onSelectSegment={setSegmentRequest}
            query={query}
          />
        </div>
        <div className="timeline-view__detail-pane">
          {selectedRecord && (
            <>
              <button className="timeline-view__back" onClick={() => setViewMode('list')}>
                <ArrowLeft size={16} aria-hidden="true" />返回会话列表
              </button>
              <TranscriptView
                record={selectedRecord}
                query={query}
                segmentRequest={segmentRequest}
                onRevisionChange={handleRevisionChange}
                onExportMarkdown={exportRecordMarkdown}
                onNotice={onNotice}
              />
            </>
          )}
        </div>
      </div>}

      <StatsBar />
    </main>
  )
}
