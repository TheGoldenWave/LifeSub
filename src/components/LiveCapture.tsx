import { useState, useEffect } from 'react'
import { CirclePause, Square, Plus, Copy } from 'lucide-react'
import { NotePanel } from './NotePanel'
import { loadNotes, createNoteAdapter, deleteNoteAdapter } from '../data/adapter'
import type { CaptureMode, CaptureState, LiveSegment, CaptureNote } from '../domain'

interface LiveCaptureProps {
  onNotice: (msg: string) => void
}

const DEMO_SEGMENTS: LiveSegment[] = [
  { id: 'ls-1', startMs: 12000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '我们今天先确认首版范围，重点是把基础闭环真正跑起来。', completed: true },
  { id: 'ls-2', startMs: 18000, speaker: { id: 'spk-2', label: '我', source: 'manual', voiceprintId: null }, text: '好的，我记一下。证据链要保证每次修改都能追溯。', completed: true },
  { id: 'ls-3', startMs: 25000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '对，而且要保证搜索结果能回到准确的音频时间范围。', completed: true },
  { id: 'ls-4', startMs: 32000, speaker: { id: 'spk-3', label: '可能是李娜？', source: 'dictionary', voiceprintId: null }, text: '还有一个点，关于数据目录的权限控制...', completed: true },
  { id: 'ls-5', startMs: 45000, speaker: { id: 'spk-4', label: '未知说话人 1', source: 'unknown', voiceprintId: null }, text: '这个方案我觉得可以，但是需要再确认一下安全性...', completed: false },
]

export function LiveCapture({ onNotice }: LiveCaptureProps) {
  const [captureState, setCaptureState] = useState<CaptureState>('idle')
  const [captureMode, setCaptureMode] = useState<CaptureMode>('smart')
  const [segments, setSegments] = useState<LiveSegment[]>([])
  const [notes, setNotes] = useState<CaptureNote[]>([])
  const [showDemo, setShowDemo] = useState(false)

  useEffect(() => {
    loadNotes('current').then((loaded) => {
      if (loaded.length > 0) setNotes(loaded)
    })
  }, [])

  const formatTime = (ms: number) => {
    const s = Math.floor(ms / 1000)
    return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
  }

  const startCapture = () => {
    setCaptureState('recording')
    setSegments([])
    setNotes([])
    setTimeout(() => {
      setSegments(DEMO_SEGMENTS)
      setShowDemo(true)
    }, 1000)
  }

  const stopCapture = () => {
    setCaptureState('stopped')
    onNotice('录音已保存，可在时间线页面查看。')
  }

  const addNote = (note: CaptureNote) => {
    setNotes((prev) => [...prev, note].sort((a, b) => a.timestampMs - b.timestampMs))
    createNoteAdapter('current', note.content, note.timestampMs, note.tag, note.segmentId)
  }

  const deleteNote = (id: string) => {
    setNotes((prev) => prev.filter((n) => n.id !== id))
    deleteNoteAdapter(id)
  }

  const copyAll = async () => {
    const text = segments.map((s) => `[${formatTime(s.startMs)}] ${s.speaker.label} ${s.text}`).join('\n\n')
    await navigator.clipboard.writeText(text)
    onNotice('转写全文已复制。')
  }

  const renameSpeaker = (segmentId: string, newLabel: string) => {
    setSegments((prev) =>
      prev.map((s) =>
        s.id === segmentId
          ? { ...s, speaker: { ...s.speaker, label: newLabel, source: 'manual' as const } }
          : s
      )
    )
  }

  return (
    <main className="live-capture">
      <header className="live-capture__bar">
        <div className="live-capture__status">
          <span className={`recorder__pulse recorder__pulse--${captureState}`} aria-hidden="true" />
          <div>
            <strong>
              {captureState === 'idle' ? '准备就绪' : captureState === 'recording' ? '正在记录' : captureState === 'paused' ? '已暂停' : '记录已封存'}
            </strong>
            <small>{captureMode === 'smart' ? '智能路由 · 单声道 · 未检测到通话' : '仅麦克风'}</small>
          </div>
        </div>
        <div className="live-capture__asr">
          SenseVoice · 中文 · ITN 开启
        </div>
        <div className="live-capture__actions">
          {captureState === 'idle' && (
            <button className="button button--primary" onClick={startCapture}>
              <span className="recorder__pulse recorder__pulse--recording" /> 开始记录
            </button>
          )}
          {captureState === 'recording' && (
            <>
              <button className="button" onClick={() => setCaptureState('paused')}>
                <CirclePause size={17} />暂停
              </button>
              <button className="button" onClick={() => addNote({
                id: `note-${Date.now()}`,
                content: '',
                timestampMs: Date.now() % 3600000,
                tag: '备忘',
                segmentId: null,
                createdAt: new Date().toISOString(),
              })}>
                <Plus size={17} />笔记
              </button>
              <button className="button button--danger" onClick={stopCapture}>
                <Square size={15} />停止
              </button>
            </>
          )}
        </div>
      </header>

      <div className="live-capture__body">
        <section className="live-capture__transcript">
          <div className="live-capture__transcript-header">
            <span className="eyebrow">实时转写</span>
            {segments.length > 0 && (
              <button className="text-button" onClick={copyAll}>
                <Copy size={14} />复制全部
              </button>
            )}
          </div>

          {segments.length === 0 && !showDemo && captureState === 'idle' && (
            <div className="empty-state">
              <strong>💡 尚未开始录音</strong>
              <p>点击「开始记录」或按 ⌘R 启动实时转写</p>
            </div>
          )}
          {segments.length === 0 && captureState === 'recording' && (
            <div className="empty-state">
              <strong>🎤 正在监听...</strong>
              <p>检测到语音后将自动开始转写</p>
            </div>
          )}

          <div className="live-capture__segments">
            {segments.map((seg) => (
              <article key={seg.id} className="live-segment">
                <div className="live-segment__header">
                  <span
                    className={`live-segment__speaker live-segment__speaker--${seg.speaker.source}`}
                    title={
                      seg.speaker.source === 'unknown'
                        ? '点击重命名说话人'
                        : seg.speaker.source === 'voiceprint'
                          ? '来自声纹库'
                          : seg.speaker.source === 'dictionary'
                            ? '来自词典（点击确认）'
                            : '手动标注'
                    }
                    onClick={() => {
                      if (seg.speaker.source === 'unknown' || seg.speaker.source === 'dictionary') {
                        const name = window.prompt('重命名说话人：', seg.speaker.source === 'dictionary' ? seg.speaker.label.replace('可能是', '').replace('？', '') : '')
                        if (name) renameSpeaker(seg.id, name)
                      }
                    }}
                  >
                    {seg.speaker.label}
                  </span>
                  <span className="live-segment__time">{formatTime(seg.startMs)}</span>
                  {!seg.completed && <span className="live-segment__cursor">▌</span>}
                </div>
                <p className="live-segment__text">{seg.text}</p>
              </article>
            ))}
          </div>
        </section>

        <NotePanel
          notes={notes}
          onAdd={addNote}
          onDelete={deleteNote}
          segments={segments}
        />
      </div>
    </main>
  )
}