import { useState, useEffect, useRef } from 'react'
import { CirclePause, Square, Plus, Copy, Sparkles, Mic, Monitor, Cpu, HardDrive } from 'lucide-react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { NotePanel } from './NotePanel'
import { createNoteAdapter, deleteNoteAdapter } from '../data/adapter'
import { createCapture, startStreamingCapture, stopStreamingCapture, pauseStreamingCapture, resumeStreamingCapture, transitionCapture, isTauriRuntime, llmPolish, registerQuickInputHotkey, type CoreCaptureSession } from '../services/lifesub'
import type { CaptureMode, CaptureState, LiveSegment, CaptureNote } from '../domain'

interface LiveCaptureProps {
  onNotice: (msg: string) => void
}

interface StreamingErrorPayload {
  code: string
  message: string
}

const DEMO_SEGMENTS: LiveSegment[] = [
  { id: 'ls-1', startMs: 12000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '我们今天先确认首版范围，重点是把基础闭环真正跑起来。', completed: true },
  { id: 'ls-2', startMs: 18000, speaker: { id: 'spk-2', label: '我', source: 'manual', voiceprintId: null }, text: '好的，我记一下。证据链要保证每次修改都能追溯。', completed: true },
  { id: 'ls-3', startMs: 25000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '对，而且要保证搜索结果能回到准确的音频时间范围。', completed: true },
  { id: 'ls-4', startMs: 32000, speaker: { id: 'spk-3', label: '可能是李娜？', source: 'dictionary', voiceprintId: null }, text: '还有一个点，关于数据目录的权限控制...', completed: true },
  { id: 'ls-5', startMs: 45000, speaker: { id: 'spk-4', label: '未知说话人 1', source: 'unknown', voiceprintId: null }, text: '这个方案我觉得可以，但是需要再确认一下安全性...', completed: false },
]

const BROWSER_DEMO_NOTICE = '浏览器演示数据，不会录音或保存。'
const DESKTOP_STREAMING_ERROR = '桌面实时采集未接通，已阻止演示数据回退。'
const NOTE_SAVE_FAILURE_NOTICE = '笔记保存失败，请稍后重试。'
const NOTE_EMPTY_NOTICE = '笔记内容不能为空。'
const NOTE_ENTRY_NOTICE = '请在右侧输入笔记内容后保存。'
const POLISH_FAILURE_NOTICE = '润色失败，请检查本地 LLM 是否可用。'
const STOPPED_NOTICE = '记录已停止，尚未确认持久化。'
const PAUSE_FAILURE_NOTICE = '暂停失败，当前会话仍保持录制中。'
const RESUME_FAILURE_NOTICE = '继续失败，当前会话仍保持暂停。'

function rulePolish(text: string) {
  const fillerWords = ['呃', '啊', '那个', '就是说', '然后', '嗯', '这个']
  let result = text
  for (const word of fillerWords) {
    result = result.replaceAll(word, '')
  }
  return result.replace(/\s{2,}/g, ' ').trim()
}

export function LiveCapture({ onNotice }: LiveCaptureProps) {
  const desktopRuntime = isTauriRuntime()
  const [captureState, setCaptureState] = useState<CaptureState>('idle')
  const [_captureMode, _setCaptureMode] = useState<CaptureMode>('smart')
  const [segments, setSegments] = useState<LiveSegment[]>([])
  const [notes, setNotes] = useState<CaptureNote[]>([])
  const [showPolished, setShowPolished] = useState(false)
  const [polishedTexts, setPolishedTexts] = useState<Record<string, string>>({})
  const [polishing, setPolishing] = useState(false)
  const [quickInputActive, setQuickInputActive] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [lifecycleBusy, setLifecycleBusy] = useState(false)
  const unlistenRef = useRef<UnlistenFn | null>(null)
  const quickInputUnlistenRef = useRef<UnlistenFn | null>(null)
  const currentSessionRef = useRef<CoreCaptureSession | null>(null)
  const captureStartedAtRef = useRef<number | null>(null)
  const captureStateRef = useRef<CaptureState>('idle')
  const activeStartAttemptRef = useRef(0)
  const startInFlightRef = useRef(false)
  const lifecycleBusyRef = useRef(false)
  const lifecycleGenerationRef = useRef(0)
  const browserDemoTimerRef = useRef<number | null>(null)
  const notePanelRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const handleKeydown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase()
      const target = event.target instanceof HTMLElement ? event.target : null
      const isEditable = Boolean(
        target?.closest('input, textarea, select, [contenteditable="true"], [contenteditable=""], [role="textbox"]'),
      )
      const insideDialog = Boolean(target?.closest('[role="dialog"], dialog, [aria-modal="true"]'))
      if (!event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || key !== 'r' || isEditable || insideDialog) {
        return
      }
      if (captureStateRef.current === 'idle' || captureStateRef.current === 'stopped') {
        event.preventDefault()
        void startCapture()
      }
    }

    window.addEventListener('keydown', handleKeydown)
    return () => {
      window.removeEventListener('keydown', handleKeydown)
      clearBrowserDemoTimer()
      cleanupListeners()
    }
  }, [])

  useEffect(() => {
    captureStateRef.current = captureState
  }, [captureState])

  const formatTime = (ms: number) => {
    const s = Math.floor(ms / 1000)
    return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
  }

  const cleanupListeners = () => {
    if (unlistenRef.current) {
      unlistenRef.current()
      unlistenRef.current = null
    }
    if (quickInputUnlistenRef.current) {
      quickInputUnlistenRef.current()
      quickInputUnlistenRef.current = null
    }
  }

  const clearBrowserDemoTimer = () => {
    if (browserDemoTimerRef.current !== null) {
      window.clearTimeout(browserDemoTimerRef.current)
      browserDemoTimerRef.current = null
    }
  }

  const setLifecycleBusyState = (busy: boolean) => {
    lifecycleBusyRef.current = busy
    setLifecycleBusy(busy)
  }

  const currentSessionMatches = (sessionId: string | null) =>
    sessionId !== null && currentSessionRef.current?.id === sessionId

  const stopCurrentSession = async () => {
    if (!desktopRuntime || !currentSessionRef.current) {
      return
    }
    if (currentSessionRef.current.state !== 'recording' && currentSessionRef.current.state !== 'paused') {
      return
    }
    try {
      currentSessionRef.current = await transitionCapture(currentSessionRef.current, 'stopped')
    } catch {
      setErrorMessage('当前会话停止失败，请检查本地数据目录。')
      onNotice('当前会话停止失败，请检查本地数据目录。')
    }
  }

  const finalizeSessionAsStopped = async (session: CoreCaptureSession | null) => {
    if (!desktopRuntime || !session) {
      return
    }

    try {
      let currentSession = session
      if (currentSession.state === 'idle') {
        currentSession = await transitionCapture(currentSession, 'recording')
      }
      if (currentSession.state === 'recording' || currentSession.state === 'paused') {
        await transitionCapture(currentSession, 'stopped')
      }
    } catch {
      setErrorMessage('失败会话清理未完成，请检查本地数据目录。')
      onNotice('失败会话清理未完成，请检查本地数据目录。')
    }
  }

  const currentNoteTimestamp = () => {
    if (captureStartedAtRef.current === null) {
      return 0
    }
    return Math.max(0, Math.round(performance.now() - captureStartedAtRef.current))
  }

  const resetCaptureBuffers = () => {
    clearBrowserDemoTimer()
    setSegments([])
    setNotes([])
    setShowPolished(false)
    setPolishedTexts({})
    setQuickInputActive(false)
    setErrorMessage(null)
  }

  const beginFailedAttempt = (attemptId: number, message: string) => {
    if (activeStartAttemptRef.current !== attemptId) {
      return
    }

    lifecycleGenerationRef.current += 1
    activeStartAttemptRef.current = 0
    startInFlightRef.current = false
    setLifecycleBusyState(false)
    cleanupListeners()
    setCaptureState('idle')
    setErrorMessage(message)
    onNotice(message)
    void stopStreamingCapture().catch(() => undefined)
    const failedSession = currentSessionRef.current
    currentSessionRef.current = null
    void finalizeSessionAsStopped(failedSession)
  }

  const startCapture = async () => {
    if (
      startInFlightRef.current ||
      lifecycleBusyRef.current ||
      (captureStateRef.current !== 'idle' && captureStateRef.current !== 'stopped')
    ) {
      return
    }

    startInFlightRef.current = true
    setLifecycleBusyState(true)
    const lifecycleGeneration = lifecycleGenerationRef.current + 1
    lifecycleGenerationRef.current = lifecycleGeneration
    cleanupListeners()
    resetCaptureBuffers()
    captureStartedAtRef.current = performance.now()
    const attemptId = activeStartAttemptRef.current + 1
    activeStartAttemptRef.current = attemptId

    if (desktopRuntime) {
      try {
        const session = await createCapture(`实时记录 ${new Date().toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })}`)
        if (activeStartAttemptRef.current !== attemptId) {
          return
        }
        currentSessionRef.current = session

        const unlisten = await listen<LiveSegment>('asr-live-segment', (event) => {
          if (activeStartAttemptRef.current !== attemptId && captureStateRef.current === 'idle') {
            return
          }
          setSegments((prev) => [...prev, event.payload].sort((a, b) => a.startMs - b.startMs))
        })
        const unlistenError = await listen<StreamingErrorPayload>('asr-live-error', (event) => {
          beginFailedAttempt(attemptId, event.payload.message)
        })
        unlistenRef.current = () => {
          unlisten()
          unlistenError()
        }

        try {
          await registerQuickInputHotkey()
          const qiUnlisten = await listen<{ active: boolean }>('quick-input-started', () => {
            setQuickInputActive(true)
          })
          const qiStopUnlisten = await listen<{ active: boolean }>('quick-input-stopped', () => {
            setQuickInputActive(false)
          })
          quickInputUnlistenRef.current = () => { qiUnlisten(); qiStopUnlisten() }
        } catch {
          setQuickInputActive(false)
        }

        await startStreamingCapture()
        if (
          activeStartAttemptRef.current !== attemptId ||
          lifecycleGenerationRef.current !== lifecycleGeneration ||
          !currentSessionRef.current
        ) {
          return
        }
        const sessionId = currentSessionRef.current.id
        const recordingSession = await transitionCapture(currentSessionRef.current, 'recording')
        if (
          activeStartAttemptRef.current !== attemptId ||
          lifecycleGenerationRef.current !== lifecycleGeneration ||
          !currentSessionMatches(sessionId)
        ) {
          startInFlightRef.current = false
          setLifecycleBusyState(false)
          void finalizeSessionAsStopped(recordingSession)
          return
        }
        currentSessionRef.current = recordingSession
        startInFlightRef.current = false
        setLifecycleBusyState(false)
        setCaptureState('recording')
      } catch {
        beginFailedAttempt(attemptId, DESKTOP_STREAMING_ERROR)
      }
    } else {
      startInFlightRef.current = false
      setLifecycleBusyState(false)
      setCaptureState('recording')
      onNotice(BROWSER_DEMO_NOTICE)
      browserDemoTimerRef.current = window.setTimeout(() => {
        setSegments(DEMO_SEGMENTS)
        browserDemoTimerRef.current = null
      }, 1000)
    }
  }

  const stopCapture = async () => {
    if (lifecycleBusyRef.current) {
      return
    }

    const sessionSnapshot = currentSessionRef.current
    const sessionId = sessionSnapshot?.id ?? null
    const lifecycleGeneration = lifecycleGenerationRef.current + 1
    lifecycleGenerationRef.current = lifecycleGeneration
    activeStartAttemptRef.current = 0
    startInFlightRef.current = false
    setLifecycleBusyState(true)
    cleanupListeners()
    clearBrowserDemoTimer()
    setCaptureState('stopped')
    setQuickInputActive(false)

    try {
      await stopStreamingCapture()
    } catch {
      // ignore: browser demo and failed starts do not keep a desktop stream alive
    }
    if (
      sessionSnapshot &&
      lifecycleGenerationRef.current === lifecycleGeneration &&
      currentSessionMatches(sessionId)
    ) {
      await stopCurrentSession()
    }
    if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
      currentSessionRef.current = null
    }
    if (lifecycleGenerationRef.current === lifecycleGeneration) {
      setLifecycleBusyState(false)
    }
    onNotice(desktopRuntime ? STOPPED_NOTICE : BROWSER_DEMO_NOTICE)
  }

  const polishAll = async () => {
    if (segments.length === 0) return
    setPolishing(true)
    const fullText = segments.map((s) => s.text).join('\n')
    try {
      if (!desktopRuntime) {
        const polishedLines = segments.map((segment) => rulePolish(segment.text))
        const demoPolished: Record<string, string> = {}
        segments.forEach((segment, index) => {
          demoPolished[segment.id] = polishedLines[index] ?? segment.text
        })
        setPolishedTexts(demoPolished)
        setShowPolished(true)
        onNotice('演示模式仅执行规则清理，不会调用本地 LLM。')
        return
      }

      const result = await llmPolish({ text: fullText })
      if (result.provider !== 'ollama' || result.fallback || result.error) {
        throw new Error(POLISH_FAILURE_NOTICE)
      }
      const normalized = result.polished.trim()
      if (!normalized) {
        throw new Error(POLISH_FAILURE_NOTICE)
      }

      const polishedLines = normalized.split('\n')
      const newPolished: Record<string, string> = {}
      segments.forEach((s, i) => {
        newPolished[s.id] = polishedLines[i] ?? s.text
      })
      setPolishedTexts(newPolished)
      setShowPolished(true)
      onNotice('本地 LLM 润色完成。')
    } catch {
      onNotice(POLISH_FAILURE_NOTICE)
    }
    setPolishing(false)
  }

  const togglePause = async () => {
    if (!desktopRuntime) {
      setCaptureState((state) => state === 'recording' ? 'paused' : 'recording')
      return
    }
    if (lifecycleBusyRef.current || !currentSessionRef.current) {
      return
    }

    const sessionId = currentSessionRef.current.id
    const lifecycleGeneration = lifecycleGenerationRef.current
    setLifecycleBusyState(true)
    if (captureState === 'recording') {
      setCaptureState('paused')
      let pausedSession: CoreCaptureSession | null = null
      try {
        if (desktopRuntime && currentSessionRef.current) {
          pausedSession = await transitionCapture(currentSessionRef.current, 'paused')
        }
        await pauseStreamingCapture()
        if (
          pausedSession &&
          lifecycleGenerationRef.current === lifecycleGeneration &&
          currentSessionMatches(sessionId)
        ) {
          currentSessionRef.current = pausedSession
        }
        if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
          setLifecycleBusyState(false)
        }
      } catch {
        if (
          pausedSession &&
          lifecycleGenerationRef.current === lifecycleGeneration &&
          currentSessionMatches(sessionId)
        ) {
          let resumedSession: CoreCaptureSession | null = null
          try {
            resumedSession = await transitionCapture(pausedSession, 'recording')
          } catch {
            resumedSession = pausedSession
          }
          if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
            currentSessionRef.current = resumedSession
          }
        }
        try {
          await resumeStreamingCapture()
        } catch {
          // ignore rollback failure; visible notice is more important than hidden state
        }
        if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
          setCaptureState('recording')
          setLifecycleBusyState(false)
          onNotice(PAUSE_FAILURE_NOTICE)
        }
      }
    } else {
      setCaptureState('recording')
      let recordingSession: CoreCaptureSession | null = null
      try {
        if (desktopRuntime && currentSessionRef.current) {
          recordingSession = await transitionCapture(currentSessionRef.current, 'recording')
        }
        await resumeStreamingCapture()
        if (
          recordingSession &&
          lifecycleGenerationRef.current === lifecycleGeneration &&
          currentSessionMatches(sessionId)
        ) {
          currentSessionRef.current = recordingSession
        }
        if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
          setLifecycleBusyState(false)
        }
      } catch {
        if (
          recordingSession &&
          lifecycleGenerationRef.current === lifecycleGeneration &&
          currentSessionMatches(sessionId)
        ) {
          let pausedRollbackSession: CoreCaptureSession | null = null
          try {
            pausedRollbackSession = await transitionCapture(recordingSession, 'paused')
          } catch {
            pausedRollbackSession = recordingSession
          }
          if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
            currentSessionRef.current = pausedRollbackSession
          }
        }
        try {
          await pauseStreamingCapture()
        } catch {
          // ignore rollback failure; visible notice is more important than hidden state
        }
        if (lifecycleGenerationRef.current === lifecycleGeneration && currentSessionMatches(sessionId)) {
          setCaptureState('paused')
          setLifecycleBusyState(false)
          onNotice(RESUME_FAILURE_NOTICE)
        }
      }
    }
  }

  const addNote = async (note: CaptureNote) => {
    const content = note.content.trim()
    if (!content) {
      onNotice(NOTE_EMPTY_NOTICE)
      return false
    }

    const sessionId = desktopRuntime ? currentSessionRef.current?.id ?? null : 'browser-demo'
    if (!sessionId) {
      onNotice('当前会话未就绪，无法保存笔记。')
      return false
    }

    const timestampMs = currentNoteTimestamp()
    try {
      const persisted = await createNoteAdapter(sessionId, content, timestampMs, note.tag, note.segmentId)
      setNotes((prev) => [...prev, persisted].sort((a, b) => a.timestampMs - b.timestampMs))
      return true
    } catch {
      setErrorMessage(NOTE_SAVE_FAILURE_NOTICE)
      onNotice(NOTE_SAVE_FAILURE_NOTICE)
      return false
    }
  }

  const deleteNote = async (id: string) => {
    try {
      await deleteNoteAdapter(id)
      setNotes((prev) => prev.filter((n) => n.id !== id))
      return true
    } catch {
      onNotice('笔记删除失败，请稍后重试。')
      return false
    }
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

  const statusTitle = desktopRuntime
    ? captureState === 'idle'
      ? errorMessage
        ? '采集未就绪'
        : '待检测'
      : captureState === 'recording'
        ? '正在记录'
        : captureState === 'paused'
          ? '已暂停'
          : '记录已停止'
    : captureState === 'recording'
      ? '演示播放中'
      : captureState === 'paused'
        ? '演示已暂停'
        : captureState === 'stopped'
          ? '演示已停止'
          : '浏览器演示'

  const statusSubtitle = desktopRuntime
    ? errorMessage
      ? errorMessage
      : captureState === 'idle'
        ? '点击「开始记录」或按 ⌘R 启动；失败时不会展示任何演示转写。'
        : '桌面实时采集失败时会显示错误，不会回退演示数据。'
    : BROWSER_DEMO_NOTICE

  return (
    <main className="live-capture">
      <header className="live-capture__bar">
        <div className="live-capture__status">
          <span className={`recorder__pulse recorder__pulse--${captureState}`} aria-hidden="true" />
          <div>
            <strong>{statusTitle}</strong>
            <small>{statusSubtitle}</small>
          </div>
        </div>
        <div className="live-capture__asr">
          {desktopRuntime ? '本地 ASR · 待接通' : '浏览器演示 · Demo 数据'}
        </div>
        <div className="live-capture__actions">
          {captureState === 'idle' && (
            <button className="button button--primary" onClick={startCapture}>
              <span className="recorder__pulse recorder__pulse--recording" /> {desktopRuntime ? '开始记录' : '开始演示'}
            </button>
          )}
          {captureState === 'idle' && lifecycleBusy && (
            <button className="button button--primary" onClick={startCapture} disabled>
              <span className="recorder__pulse recorder__pulse--recording" /> {desktopRuntime ? '开始记录' : '开始演示'}
            </button>
          )}
          {captureState === 'recording' && (
            <>
              <button className="button" onClick={togglePause} disabled={lifecycleBusy}>
                <CirclePause size={17} />暂停
              </button>
              <button className="button" disabled={lifecycleBusy} onClick={() => {
                notePanelRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
                onNotice(NOTE_ENTRY_NOTICE)
              }}>
                <Plus size={17} />新笔记
              </button>
              <button className="button button--danger" onClick={stopCapture} disabled={lifecycleBusy}>
                <Square size={15} />停止
              </button>
            </>
          )}
          {captureState === 'paused' && (
            <>
              <button className="button button--primary" onClick={togglePause} disabled={lifecycleBusy}>
                <CirclePause size={17} />继续
              </button>
              <button className="button button--danger" onClick={stopCapture} disabled={lifecycleBusy}>
                <Square size={15} />停止
              </button>
            </>
          )}
          {captureState === 'stopped' && (
            <button className="button button--primary" onClick={startCapture} disabled={lifecycleBusy}>
              <span className="recorder__pulse recorder__pulse--recording" /> {desktopRuntime ? '开始新记录' : '重新演示'}
            </button>
          )}
        </div>
        {quickInputActive && (
          <div className="quick-input-indicator">
            <span className="recorder__pulse recorder__pulse--recording" /> 快速输入已激活
          </div>
        )}
      </header>

      <div className="live-capture__body">
        <section className="live-capture__transcript">
          <div className="live-capture__transcript-header">
            <span className="eyebrow">实时转写</span>
            {segments.length > 0 && (
              <>
                {showPolished ? (
                  <button className="text-button" onClick={() => setShowPolished(false)}>
                    查看原始
                  </button>
                ) : (
                  <button className="text-button" onClick={polishAll} disabled={polishing}>
                    <Sparkles size={14} />{polishing ? '润色中...' : 'AI 润色'}
                  </button>
                )}
                <button className="text-button" onClick={copyAll}>
                  <Copy size={14} />复制全部
                </button>
              </>
            )}
          </div>

          {segments.length === 0 && captureState === 'idle' && (
            <div className="live-capture__empty" role={errorMessage ? 'alert' : undefined}>
              <div className="live-capture__empty-icon">
                <Mic size={28} />
              </div>
              <strong>
                {desktopRuntime
                  ? errorMessage ? '采集未就绪' : '待检测'
                  : '浏览器演示未开始'}
              </strong>
              <p>
                {desktopRuntime
                  ? '点击「开始记录」或按 ⌘R 启动；失败时不会展示任何演示转写。'
                  : '点击「开始演示」或按 ⌘R 预览界面；不会录音、保存或调用本地 ASR。'}
              </p>
              {desktopRuntime && (
                <ul className="live-capture__status-list">
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <Mic size={14} />
                    <span className="live-capture__status-label">麦克风权限</span>
                    <span className="live-capture__status-value">未检测</span>
                    <span className="live-capture__status-hint">点击「开始记录」自动请求</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <Monitor size={14} />
                    <span className="live-capture__status-label">设备状态</span>
                    <span className="live-capture__status-value">待接通</span>
                    <span className="live-capture__status-hint">在偏好设置中检查输入设备</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <Cpu size={14} />
                    <span className="live-capture__status-label">Provider</span>
                    <span className="live-capture__status-value">本地 ASR 未接通</span>
                    <span className="live-capture__status-hint">在设置中确认模型已安装</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <HardDrive size={14} />
                    <span className="live-capture__status-label">保存状态</span>
                    <span className="live-capture__status-value">需等待持久化确认</span>
                    <span className="live-capture__status-hint">开始记录后自动创建 Catalog 会话</span>
                  </li>
                </ul>
              )}
              {errorMessage && <p className="live-capture__error">{errorMessage}</p>}
            </div>
          )}
          {segments.length === 0 && captureState === 'recording' && (
            <div className="empty-state">
              <strong>{desktopRuntime ? '🎤 正在监听...' : '🎬 正在准备演示...'}</strong>
              <p>{desktopRuntime ? '检测到语音后将自动开始转写' : BROWSER_DEMO_NOTICE}</p>
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
                <p className="live-segment__text">
                  {showPolished && polishedTexts[seg.id] ? polishedTexts[seg.id] : seg.text}
                </p>
              </article>
            ))}
          </div>
        </section>

        <div ref={notePanelRef}>
          <NotePanel
            notes={notes}
            onAdd={addNote}
            onDelete={deleteNote}
            segments={segments}
          />
        </div>
      </div>
    </main>
  )
}
