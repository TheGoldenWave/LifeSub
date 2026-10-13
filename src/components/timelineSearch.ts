import type { EvidenceRecord, TranscriptSegment } from '../domain'

export interface SegmentSelectionRequest {
  recordId: string
  segmentId: string
  requestId: number
}

function normalizeQuery(query: string) {
  return query.trim().toLocaleLowerCase()
}

export function formatTimelineTime(milliseconds: number) {
  const totalSeconds = Math.floor(milliseconds / 1000)
  return `${String(Math.floor(totalSeconds / 60)).padStart(2, '0')}:${String(totalSeconds % 60).padStart(2, '0')}`
}

export function segmentMatchesQuery(segment: TranscriptSegment, query: string) {
  const normalizedQuery = normalizeQuery(query)
  if (!normalizedQuery) return true

  const searchableText = [
    segment.text,
    segment.source,
    formatTimelineTime(segment.startMs),
    formatTimelineTime(segment.endMs),
  ].join(' ').toLocaleLowerCase()

  return searchableText.includes(normalizedQuery)
}

export function getRecordMatches(record: EvidenceRecord, query: string) {
  return record.revision.segments.filter((segment) => segmentMatchesQuery(segment, query))
}

export function getTimelineSearchSummary(records: EvidenceRecord[], query: string) {
  if (!query.trim()) return { sessionCount: 0, matchCount: 0 }

  return records.reduce((summary, record) => {
    const matches = getRecordMatches(record, query)
    return {
      sessionCount: summary.sessionCount + (matches.length > 0 ? 1 : 0),
      matchCount: summary.matchCount + matches.length,
    }
  }, { sessionCount: 0, matchCount: 0 })
}
