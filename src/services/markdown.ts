import type { EvidenceRecord } from '../domain'

function timestamp(milliseconds: number) {
  const totalSeconds = Math.floor(milliseconds / 1000)
  return `${String(Math.floor(totalSeconds / 60)).padStart(2, '0')}:${String(totalSeconds % 60).padStart(2, '0')}`
}

export function renderRecordMarkdown(record: EvidenceRecord) {
  const frontmatter = [
    '---',
    `record_id: ${record.id}`,
    `evidence_uri: lifesub://record/${record.id}`,
    `transcript_revision: ${record.revision.number}`,
    `asr_provider: ${record.revision.provider}`,
    '---',
    '',
    `# ${record.title}`,
  ]
  const segments = record.revision.segments.flatMap((segment) => ['', `## ${timestamp(segment.startMs)}`, '', `[${segment.source}] ${segment.text}`])
  return [...frontmatter, ...segments, ''].join('\n')
}

export function downloadMarkdown(record: EvidenceRecord) {
  const blob = new Blob([renderRecordMarkdown(record)], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `${record.title}.md`
  link.click()
  URL.revokeObjectURL(url)
}
