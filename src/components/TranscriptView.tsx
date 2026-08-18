import { CheckCircle2, Copy, Download, FilePenLine, Radio, Search } from 'lucide-react'
import { useState } from 'react'
import type { EvidenceRecord, TranscriptRevision } from '../domain'
import { downloadMarkdown } from '../services/markdown'

function formatTimestamp(milliseconds: number) {
  const totalSeconds = Math.floor(milliseconds / 1000)
  return `${String(Math.floor(totalSeconds / 60)).padStart(2, '0')}:${String(totalSeconds % 60).padStart(2, '0')}`
}

interface TranscriptViewProps {
  record: EvidenceRecord
  query: string
  onRevisionChange: (revision: TranscriptRevision) => void
  onNotice: (message: string) => void
}

export function TranscriptView({ record, query, onRevisionChange, onNotice }: TranscriptViewProps) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(record.revision.segments[0]?.text ?? '')
  const visibleSegments = record.revision.segments.filter((segment) => segment.text.toLocaleLowerCase().includes(query.toLocaleLowerCase()))
  const sources = [...new Set(record.revision.segments.map((segment) => segment.source))].join(' + ')

  const saveRevision = () => {
    const firstSegment = record.revision.segments[0]
    if (!firstSegment || !draft.trim()) return
    onRevisionChange({ number: record.revision.number + 1, provider: '人工修订', label: `人工修订 · r${record.revision.number + 1}`, segments: [{ ...firstSegment, text: draft.trim() }, ...record.revision.segments.slice(1)] })
    setEditing(false)
  }

  const copyEvidenceUri = async () => {
    await navigator.clipboard.writeText(`lifesub://record/${record.id}`)
    onNotice('Evidence URI 已复制，可交给 Malow 或其他获授权的消费者。')
  }

  return (
    <main className="transcript">
      <header className="transcript__header">
        <div><span className="eyebrow">Evidence Record</span><h1>{record.title}</h1><p>{record.startedAt} · {record.duration} · {sources}</p></div>
        <div className="transcript__tools">
          <button className="icon-button" aria-label="复制 Evidence URI" onClick={copyEvidenceUri}><Copy size={17} /></button>
          <button className="button" onClick={() => { downloadMarkdown(record); onNotice('Markdown 已按当前 revision 导出。') }}><Download size={16} />导出 Markdown</button>
        </div>
      </header>
      <div className="evidence-strip">
        <span><CheckCircle2 size={15} />证据可用</span>
        <code>lifesub://record/{record.id}</code>
        <span>{record.revision.label}</span>
        {record.revision.number > 1 && <button className="text-button" onClick={() => onRevisionChange(record.originalRevision)}>查看原始 r1</button>}
      </div>
      <section className="transcript__body">
        <div className="revision-line"><span>{record.revision.label}</span><small>{record.revision.provider}</small><button className="text-button" aria-label="创建修订" onClick={() => setEditing(true)}><FilePenLine size={15} />创建修订</button></div>
        {editing && <div className="revision-editor"><label htmlFor="revision-text">修订文本</label><textarea id="revision-text" aria-label="修订文本" value={draft} onChange={(event) => setDraft(event.target.value)} /><div><button className="text-button" onClick={() => setEditing(false)}>取消</button><button className="button button--primary" onClick={saveRevision}>保存修订</button></div></div>}
        <div className="segments">
          {visibleSegments.map((segment) => <article className="segment" key={segment.id}><button className="segment__time" aria-label={`播放 ${formatTimestamp(segment.startMs)}`}><Radio size={14} />{formatTimestamp(segment.startMs)}</button><div><span className="segment__source">{segment.source}</span><p>{segment.text}</p><small>{formatTimestamp(segment.startMs)}–{formatTimestamp(segment.endMs)} · {segment.id}</small></div></article>)}
          {visibleSegments.length === 0 && <div className="empty-state"><Search size={24} /><strong>没有匹配的原话</strong><p>换一个更短的关键词，或清除搜索查看完整记录。</p></div>}
        </div>
      </section>
    </main>
  )
}
