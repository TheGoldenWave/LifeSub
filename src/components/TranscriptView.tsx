import { AlertTriangle, CheckCircle2, ChevronDown, Copy, Download, FilePenLine, Radio, RefreshCw, Search } from 'lucide-react'
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
  onRetranscribe?: (recordId: string) => void
  /** V0.2: all available revisions for the selector */
  allRevisions?: TranscriptRevision[]
}

const PROVENANCE_LABELS: Record<string, string> = {
  legacy_unverified: '未验证来源',
  verified_local_asr: '本地 ASR 已验证',
  manual: '人工修订',
}

export function TranscriptView({ record, query, onRevisionChange, onNotice, onRetranscribe, allRevisions }: TranscriptViewProps) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(record.revision.segments[0]?.text ?? '')
  const [showRetranscribeConfirm, setShowRetranscribeConfirm] = useState(false)
  const [showRevisionMenu, setShowRevisionMenu] = useState(false)
  const visibleSegments = record.revision.segments.filter((segment) => segment.text.toLocaleLowerCase().includes(query.toLocaleLowerCase()))
  const sources = [...new Set(record.revision.segments.map((segment) => segment.source))].join(' + ')
  const revisions = allRevisions ?? [record.originalRevision, record.revision].filter((r, i, arr) => arr.findIndex((x) => x.number === r.number) === i)
  const provenance = record.revision.provenance ?? 'legacy_unverified'
  const isCorrupted = record.chunkIntegrity === 'corrupted' || record.chunkIntegrity === 'missing'

  const saveRevision = () => {
    const firstSegment = record.revision.segments[0]
    if (!firstSegment || !draft.trim()) return
    onRevisionChange({ number: record.revision.number + 1, provider: '人工修订', label: `人工修订 · r${record.revision.number + 1}`, segments: [{ ...firstSegment, text: draft.trim() }, ...record.revision.segments.slice(1)], provenance: 'manual' })
    setEditing(false)
  }

  const copyEvidenceUri = async () => {
    await navigator.clipboard.writeText(`lifesub://record/${record.id}`)
    onNotice('Evidence URI 已复制，可交给 Malow 或其他获授权的消费者。')
  }

  const handleRetranscribe = () => {
    setShowRetranscribeConfirm(false)
    onRetranscribe?.(record.id)
    onNotice('已提交重新转写请求，请等待本地 ASR 处理完成。')
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
        {isCorrupted ? (
          <span className="evidence-strip__warning"><AlertTriangle size={15} />来源不可用</span>
        ) : (
          <span><CheckCircle2 size={15} />证据可用</span>
        )}
        <code>lifesub://record/{record.id}</code>
        <span>{record.revision.label}</span>
        {record.revision.number > 1 && <button className="text-button" onClick={() => onRevisionChange(record.originalRevision)}>查看原始 r1</button>}

        {revisions.length > 1 && (
          <div className="revision-selector">
            <button className="text-button" onClick={() => setShowRevisionMenu(!showRevisionMenu)} aria-label="切换修订版本">
              <ChevronDown size={14} />
            </button>
            {showRevisionMenu && (
              <div className="revision-selector__menu" role="menu">
                {revisions.map((rev) => (
                  <button
                    key={rev.number}
                    className={`revision-selector__item ${rev.number === record.revision.number ? 'revision-selector__item--active' : ''}`}
                    role="menuitem"
                    onClick={() => { onRevisionChange(rev); setShowRevisionMenu(false) }}
                  >
                    {rev.label}
                    <small>{rev.provider}</small>
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      <section className="transcript__body">
        <div className="revision-line">
          <span>{record.revision.label}</span>
          <small>{record.revision.provider}</small>

          {record.revision.provenance && (
            <span className={`provenance-badge provenance-badge--${record.revision.provenance}`}>
              {PROVENANCE_LABELS[record.revision.provenance] ?? record.revision.provenance}
            </span>
          )}

          {record.revision.receiptIds && record.revision.receiptIds.length > 0 && (
            <small className="receipt-ref">Receipt: {record.revision.receiptIds.join(', ')}</small>
          )}

          <button className="text-button" aria-label="创建修订" onClick={() => setEditing(true)}><FilePenLine size={15} />创建修订</button>

          {onRetranscribe && (
            <button className="text-button" aria-label="重新转写" onClick={() => setShowRetranscribeConfirm(true)}>
              <RefreshCw size={15} />重新转写
            </button>
          )}
        </div>

        {showRetranscribeConfirm && (
          <div className="revision-editor" role="dialog" aria-label="确认重新转写">
            <div className="retranscribe-confirm">
              <strong>确认重新转写</strong>
              <p>将使用当前 ASR 设置重新处理此音频。成功后将添加新的修订版本，当前版本不受影响。</p>
              <div className="retranscribe-confirm__details">
                <span>Provider: {record.revision.provider}</span>
                <span>来源: {sources}</span>
              </div>
              <div className="retranscribe-confirm__actions">
                <button className="text-button" onClick={() => setShowRetranscribeConfirm(false)}>取消</button>
                <button className="button button--primary" onClick={handleRetranscribe}>确认重新转写</button>
              </div>
            </div>
          </div>
        )}

        {editing && <div className="revision-editor"><label htmlFor="revision-text">修订文本</label><textarea id="revision-text" aria-label="修订文本" value={draft} onChange={(event) => setDraft(event.target.value)} /><div><button className="text-button" onClick={() => setEditing(false)}>取消</button><button className="button button--primary" onClick={saveRevision}>保存修订</button></div></div>}

        <div className="segments">
          {visibleSegments.map((segment) => <article className="segment" key={segment.id}><button className="segment__time" aria-label={`播放 ${formatTimestamp(segment.startMs)}`}><Radio size={14} />{formatTimestamp(segment.startMs)}</button><div><span className="segment__source">{segment.source}</span><p>{segment.text}</p><small>{formatTimestamp(segment.startMs)}–{formatTimestamp(segment.endMs)} · {segment.id}</small></div></article>)}
          {visibleSegments.length === 0 && <div className="empty-state"><Search size={24} /><strong>没有匹配的原话</strong><p>换一个更短的关键词，或清除搜索查看完整记录。</p></div>}
        </div>
      </section>
    </main>
  )
}