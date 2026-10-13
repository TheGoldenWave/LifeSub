import { AlertCircle, CheckCircle2, ChevronLeft, ChevronRight, Copy, Download, FilePenLine, FileText, Pause, Play, Radio, Search } from 'lucide-react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { EvidenceRecord } from '../domain'
import { isTauriRuntime } from '../services/lifesub'
import { formatTimelineTime, segmentMatchesQuery, type SegmentSelectionRequest } from './timelineSearch'

const formatTimestamp = formatTimelineTime

interface TranscriptViewProps {
  record: EvidenceRecord
  query: string
  segmentRequest?: SegmentSelectionRequest | null
  onRevisionChange: (draft: string) => void | Promise<void>
  onExportMarkdown: (record: EvidenceRecord, revisionNumber: number) => Promise<boolean>
  onNotice: (message: string) => void
}

export function TranscriptView({ record, query, segmentRequest, onRevisionChange, onExportMarkdown, onNotice }: TranscriptViewProps) {
  const [editing, setEditing] = useState(false)
  const [selectedRevisionNumber, setSelectedRevisionNumber] = useState(record.revision.number)
  const [draft, setDraft] = useState(record.revision.segments[0]?.text ?? '')
  const [playbackSegmentId, setPlaybackSegmentId] = useState<string | null>(null)
  const [isPlaying, setIsPlaying] = useState(false)
  const [playbackPositionMs, setPlaybackPositionMs] = useState(0)
  const [playbackDurationMs, setPlaybackDurationMs] = useState(0)
  const [exporting, setExporting] = useState(false)
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0)
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const playbackEndMsRef = useRef<number | null>(null)
  const segmentRefs = useRef(new Map<string, HTMLElement>())

  const revisionHistory = useMemo(() => {
    if (record.revisions.length > 0) return record.revisions
    if (record.originalRevision.number === record.revision.number) return [record.revision]
    return [record.originalRevision, record.revision]
  }, [record])

  const activeRevision = revisionHistory.find((revision) => revision.number === selectedRevisionNumber) ?? record.revision
  const isHistoricalView = activeRevision.number !== (revisionHistory.at(-1)?.number ?? record.revision.number)
  const visibleSegments = useMemo(
    () => activeRevision.segments.filter((segment) => segmentMatchesQuery(segment, query)),
    [activeRevision, query]
  )
  const sources = [...new Set(activeRevision.segments.map((segment) => segment.source))].join(' + ')
  const hasActiveQuery = query.trim().length > 0
  const hasSegments = activeRevision.segments.length > 0

  useEffect(() => {
    setSelectedRevisionNumber(record.revision.number)
    setDraft(record.revision.segments[0]?.text ?? '')
    setEditing(false)
    setPlaybackSegmentId(null)
    setPlaybackPositionMs(0)
    setPlaybackDurationMs(0)
    setIsPlaying(false)
    if (audioRef.current) {
      audioRef.current.pause()
      audioRef.current.removeAttribute('src')
    }
  }, [record])

  useEffect(() => {
    setCurrentMatchIndex(0)
  }, [query, activeRevision.number, record.id])

  useEffect(() => {
    if (!hasActiveQuery) return
    const currentSegment = visibleSegments[currentMatchIndex]
    segmentRefs.current.get(currentSegment?.id ?? '')?.scrollIntoView?.({ block: 'nearest' })
  }, [currentMatchIndex, hasActiveQuery, visibleSegments])

  const clearPlayback = () => {
    const audio = audioRef.current
    audio?.pause()
    audio?.removeAttribute('src')
    playbackEndMsRef.current = null
    setPlaybackSegmentId(null)
    setPlaybackPositionMs(0)
    setPlaybackDurationMs(0)
    setIsPlaying(false)
  }

  const selectRevision = (revisionNumber: number) => {
    clearPlayback()
    setSelectedRevisionNumber(revisionNumber)
  }

  const saveRevision = async () => {
    const firstSegment = (revisionHistory.at(-1) ?? record.revision).segments[0]
    if (!firstSegment || !draft.trim()) return
    await onRevisionChange(draft.trim())
    setEditing(false)
  }

  const copyEvidenceUri = async () => {
    await navigator.clipboard.writeText(`lifesub://record/${record.id}`)
    onNotice('Evidence URI 已复制，可交给 Malow 或其他获授权的消费者。')
  }

  const exportMarkdown = async () => {
    setExporting(true)
    try {
      if (await onExportMarkdown(record, activeRevision.number)) {
        onNotice('Markdown 已按当前 revision 导出。')
      }
    } catch {
      onNotice('Markdown 导出失败，请重试。')
    } finally {
      setExporting(false)
    }
  }

  const handlePlayback = async (
    segment: EvidenceRecord['revision']['segments'][number],
    options: { silentUnavailable?: boolean } = {}
  ) => {
    const selectedChunk = segment.chunkId
      ? record.chunks.find((chunk) => chunk.id === segment.chunkId)
      : record.chunks.length === 1
        ? record.chunks[0]
        : undefined
    if (!selectedChunk) {
      if (!options.silentUnavailable) onNotice('找不到这条记录对应的音频文件。')
      return
    }
    if (selectedChunk.integrityState !== 'available') {
      if (!options.silentUnavailable) {
        onNotice(`音频文件不可用：${selectedChunk.errorCode ?? selectedChunk.integrityState}`)
      }
      return
    }
    const audio = audioRef.current
    if (!audio) return
    const audioSrc = isTauriRuntime() ? convertFileSrc(selectedChunk.audioPath) : selectedChunk.audioPath
    const playbackStartMs = segment.chunkStartMs ?? segment.startMs
    const playbackEndMs = segment.chunkEndMs ?? segment.endMs

    if (playbackSegmentId === segment.id) {
      if (isPlaying) {
        audio.pause()
        setIsPlaying(false)
      } else {
        try {
          await audio.play()
          setIsPlaying(true)
        } catch {
          onNotice('音频播放失败，请检查文件权限或格式。')
        }
      }
      return
    }

    playbackEndMsRef.current = playbackEndMs
    setPlaybackSegmentId(segment.id)
    audio.src = audioSrc
    audio.currentTime = playbackStartMs / 1000
    setPlaybackPositionMs(playbackStartMs)
    setPlaybackDurationMs(playbackEndMs)
    try {
      await audio.play()
      setIsPlaying(true)
    } catch {
      onNotice('音频播放失败，请检查文件权限或格式。')
    }
  }

  useEffect(() => {
    if (!segmentRequest || segmentRequest.recordId !== record.id) return
    const segment = activeRevision.segments.find((candidate) => candidate.id === segmentRequest.segmentId)
    if (!segment) return
    segmentRefs.current.get(segment.id)?.scrollIntoView?.({ block: 'center' })
    void handlePlayback(segment, { silentUnavailable: true })
  }, [segmentRequest?.requestId, record.id, activeRevision])

  const playbackStatus = playbackSegmentId
    ? `${isPlaying ? '播放中' : '已暂停'} ${formatTimestamp(playbackPositionMs)} / ${formatTimestamp(playbackDurationMs)}`
    : ''
  const playbackSegment = activeRevision.segments.find((segment) => segment.id === playbackSegmentId)

  const togglePersistentPlayback = async () => {
    const audio = audioRef.current
    if (!audio) return
    if (isPlaying) {
      audio.pause()
      setIsPlaying(false)
      return
    }
    try {
      await audio.play()
      setIsPlaying(true)
    } catch {
      onNotice('音频播放失败，请检查文件权限或格式。')
    }
  }

  const moveMatch = (direction: -1 | 1) => {
    setCurrentMatchIndex((current) => (current + direction + visibleSegments.length) % visibleSegments.length)
  }

  return (
    <main className="transcript">
      <header className="transcript__header">
        <div><span className="eyebrow">Evidence Record</span><h1>{record.title}</h1><p>{record.startedAt} · {record.duration} · {sources}</p></div>
        <div className="transcript__tools">
          <button className="icon-button" aria-label="复制 Evidence URI" title="复制 Evidence URI" onClick={copyEvidenceUri}><Copy size={17} aria-hidden="true" /></button>
          <button className="button" disabled={exporting} aria-busy={exporting} onClick={() => void exportMarkdown()}><Download size={16} aria-hidden="true" />{exporting ? '正在导出' : '导出 Markdown'}</button>
        </div>
      </header>
      <div className="evidence-strip">
        <span><CheckCircle2 size={15} />证据可用</span>
        <code>lifesub://record/{record.id}</code>
        <span>{activeRevision.label}</span>
        {revisionHistory.map((revision) => (
          <button
            key={revision.number}
            className="text-button"
            disabled={revision.number === activeRevision.number}
            onClick={() => selectRevision(revision.number)}
          >
            {revision.number === 1 ? '查看原始 r1' : `查看 r${revision.number}`}
          </button>
        ))}
      </div>
      <section className="transcript__body">
        <div className="revision-line"><span>{activeRevision.label}</span><small>{isHistoricalView ? '历史 revision 只读' : activeRevision.provider}</small>{!isHistoricalView && <button className="text-button" aria-label="创建修订" onClick={() => setEditing(true)}><FilePenLine size={15} />创建修订</button>}</div>
        {hasActiveQuery && visibleSegments.length > 0 && (
          <nav className="transcript__search-navigation" aria-label="转写搜索结果">
            <span aria-live="polite">{currentMatchIndex + 1} / {visibleSegments.length}</span>
            <button className="icon-button" aria-label="上一处命中" title="上一处命中" disabled={visibleSegments.length < 2} onClick={() => moveMatch(-1)}>
              <ChevronLeft size={16} aria-hidden="true" />
            </button>
            <button className="icon-button" aria-label="下一处命中" title="下一处命中" disabled={visibleSegments.length < 2} onClick={() => moveMatch(1)}>
              <ChevronRight size={16} aria-hidden="true" />
            </button>
          </nav>
        )}
        {editing && <div className="revision-editor"><label htmlFor="revision-text">修订文本</label><textarea id="revision-text" aria-label="修订文本" value={draft} onChange={(event) => setDraft(event.target.value)} /><div><button className="text-button" onClick={() => setEditing(false)}>取消</button><button className="button button--primary" onClick={() => void saveRevision()}>保存修订</button></div></div>}
        {playbackSegment && (
          <section className="transcript__playback-bar" aria-label="音频播放控制">
            <button className="icon-button" aria-label={isPlaying ? '暂停音频' : '继续播放音频'} onClick={() => void togglePersistentPlayback()}>
              {isPlaying ? <Pause size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
            </button>
            <div className="transcript__playback-copy">
              <strong>{playbackSegment.source}</strong>
              <span className="transcript__playback-status" role="status" aria-live="polite">{playbackStatus}</span>
            </div>
          </section>
        )}
        <audio
          ref={audioRef}
          onTimeUpdate={(event) => {
            const currentMs = Math.round(event.currentTarget.currentTime * 1000)
            setPlaybackPositionMs(currentMs)
            const playbackEndMs = playbackEndMsRef.current
            if (playbackEndMs !== null && currentMs >= playbackEndMs) {
              event.currentTarget.pause()
              setIsPlaying(false)
            }
          }}
          onPause={() => setIsPlaying(false)}
          onEnded={() => setIsPlaying(false)}
        />
        <div className="segments">
          {visibleSegments.map((segment, index) => <article ref={(node) => { if (node) segmentRefs.current.set(segment.id, node); else segmentRefs.current.delete(segment.id) }} className={`segment ${playbackSegmentId === segment.id ? 'segment--active' : ''} ${hasActiveQuery && currentMatchIndex === index ? 'segment--search-current' : ''}`} key={segment.id}><button className="segment__time" aria-pressed={playbackSegmentId === segment.id && isPlaying} aria-label={playbackSegmentId === segment.id && isPlaying ? `暂停播放 ${formatTimestamp(segment.startMs)}` : `播放 ${formatTimestamp(segment.startMs)}`} onClick={() => void handlePlayback(segment)}><Radio size={14} aria-hidden="true" />{formatTimestamp(segment.startMs)}</button><div><span className="segment__source">{segment.source}</span><p>{segment.text}</p><small>{formatTimestamp(segment.startMs)}–{formatTimestamp(segment.endMs)} · {segment.id}</small></div></article>)}
          {visibleSegments.length === 0 && hasActiveQuery && (
            <div className="empty-state"><Search size={24} /><strong>没有匹配的原话</strong><p>换一个更短的关键词，或清除搜索查看完整记录。</p></div>
          )}
          {!hasSegments && !hasActiveQuery && (
            <div className="empty-state">
              {record.status === 'processing'
                ? <><Radio size={24} /><strong>等待转写</strong><p>该记录的转写尚未完成，请稍后再查看原话内容。</p></>
                : <><FileText size={24} /><strong>暂无转写</strong><p>该记录还没有可展示的原话内容。</p></>}
            </div>
          )}
          {hasSegments && !hasActiveQuery && visibleSegments.length === 0 && (
            <div className="empty-state"><AlertCircle size={24} /><strong>转写不可用</strong><p>当前 revision 没有可展示的转写片段。</p></div>
          )}
        </div>
      </section>
    </main>
  )
}
