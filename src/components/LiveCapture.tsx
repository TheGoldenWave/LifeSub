import { useState, useEffect, useLayoutEffect, useRef } from 'react'
import { CirclePause, Square, Plus, Copy, Sparkles, Mic, Monitor, Cpu, HardDrive } from 'lucide-react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { NotePanel } from './NotePanel'
import { MicrophoneDeviceSelector } from './MicrophoneDeviceSelector'
import { createNoteAdapter, deleteNoteAdapter } from '../data/adapter'
import { createCapture, startStreamingCapture, stopStreamingCapture, pauseStreamingCapture, resumeStreamingCapture, transitionCapture, isTauriRuntime, llmPolish, registerQuickInputHotkey, getAsrSessionState, type CommittedPublication, type CoreCaptureSession } from '../services/lifesub'
import { useMicrophoneDevices, type MicrophoneAvailability } from '../microphoneDevices'
import type { CaptureMode, CaptureState, LiveSegment, CaptureNote } from '../domain'

interface LiveCaptureProps {
  onNotice: (msg: string) => void
}

interface StreamingErrorPayload {
  code: string
  message: string
}

interface DurableAsrFailurePayload {
  session_id: string
  job_id: string
  chunk_id: string
  error_code: string
  error_summary: string
  retry_available: boolean
}

const ACTIVE_LIVE_SESSION_KEY = 'lifesub.activeLiveSessionId'

const DEMO_SEGMENTS: LiveSegment[] = [
  { id: 'ls-1', startMs: 12000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '我们今天先确认首版范围，重点是把基础闭环真正跑起来。', completed: true },
  { id: 'ls-2', startMs: 18000, speaker: { id: 'spk-2', label: '我', source: 'manual', voiceprintId: null }, text: '好的，我记一下。证据链要保证每次修改都能追溯。', completed: true },
  { id: 'ls-3', startMs: 25000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '对，而且要保证搜索结果能回到准确的音频时间范围。', completed: true },
  { id: 'ls-4', startMs: 32000, speaker: { id: 'spk-3', label: '可能是李娜？', source: 'dictionary', voiceprintId: null }, text: '还有一个点，关于数据目录的权限控制...', completed: true },
  { id: 'ls-5', startMs: 45000, speaker: { id: 'spk-4', label: '未知说话人 1', source: 'unknown', voiceprintId: null }, text: '这个方案我觉得可以，但是需要再确认一下安全性...', completed: false },
]

const BROWSER_DEMO_STARTED_NOTICE = '正在播放示例转写。'
const BROWSER_DEMO_STOPPED_NOTICE = '示例转写已停止。'
const DESKTOP_STREAMING_ERROR = '桌面实时采集未接通，已阻止演示数据回退。'
const NOTE_SAVE_FAILURE_NOTICE = '笔记保存失败，请稍后重试。'
const NOTE_EMPTY_NOTICE = '笔记内容不能为空。'
const NOTE_ENTRY_NOTICE = '请在右侧输入笔记内容后保存。'
const POLISH_FAILURE_NOTICE = '润色失败，请检查本地 LLM 是否可用。'
const STOPPED_NOTICE = '记录已停止，尚未确认持久化。'
const PAUSE_FAILURE_NOTICE = '暂停失败，当前会话仍保持录制中。'
const RESUME_FAILURE_NOTICE = '继续失败，当前会话仍保持暂停。'
const RECORDING_CONFIG_FAILURE_NOTICE = '无法读取录音设置，请重试。'
const ELAPSED_REFRESH_INTERVAL_MS = 250

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
  const microphone = useMicrophoneDevices(desktopRuntime)
  const [segments, setSegments] = useState<LiveSegment[]>([])
  const [notes, setNotes] = useState<CaptureNote[]>([])
  const [showPolished, setShowPolished] = useState(false)
  const [polishedTexts, setPolishedTexts] = useState<Record<string, string>>({})
  const [polishing, setPolishing] = useState(false)
  const [quickInputActive, setQuickInputActive] = useState(false)
  const [editingSpeakerId, setEditingSpeakerId] = useState<string | null>(null)
  const [speakerDraft, setSpeakerDraft] = useState('')
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [lifecycleBusy, setLifecycleBusy] = useState(false)
  const [elapsedMs, setElapsedMs] = useState(0)
  const unlistenRef = useRef<UnlistenFn | null>(null)
  const publicationUnlistenRef = useRef<UnlistenFn | null>(null)
  const quickInputUnlistenRef = useRef<UnlistenFn | null>(null)
  const currentSessionRef = useRef<CoreCaptureSession | null>(null)
  const displaySessionIdRef = useRef<string | null>(null)
  const appliedRevisionNumberRef = useRef(0)
  const captureStartedAtRef = useRef<number | null>(null)
  const captureStateRef = useRef<CaptureState>('idle')
  const activeStartAttemptRef = useRef(0)
  const startInFlightRef = useRef(false)
  const lifecycleBusyRef = useRef(false)
  const lifecycleGenerationRef = useRef(0)
  const browserDemoTimerRef = useRef<number | null>(null)
  const elapsedAccumulatedRef = useRef(0)
  const notePanelRef = useRef<HTMLDivElement | null>(null)
  const captureInputRef = useRef<{
    mode: CaptureMode
    microphoneUid: string | null
    availability: MicrophoneAvailability
  }>({
    mode: microphone.captureMode,
    microphoneUid: microphone.selectedUid,
    availability: microphone.availability,
  })

  const applyPublication = (publication: CommittedPublication) => {
    if (publication.session_id !== displaySessionIdRef.current) return
    if (publication.revision_number <= appliedRevisionNumberRef.current) return
    appliedRevisionNumberRef.current = publication.revision_number
    setSegments(publication.segments.map((segment) => ({
      id: segment.segment_id,
      startMs: segment.start_ms,
      speaker: {
        id: segment.capture_source_id ?? segment.source,
        label: sourceLabel(segment.source),
        source: 'unknown',
        voiceprintId: null,
      },
      text: segment.text,
      completed: true,
    })))
  }

  const showDurableFailure = (failure: DurableAsrFailurePayload) => {
    if (failure.session_id !== displaySessionIdRef.current) return
    const message = failure.error_summary || `本地 ASR 失败：${failure.error_code}`
    setErrorMessage(message)
    onNotice(message)
  }

  const reconcileSession = async (sessionId: string) => {
    try {
      const state = await getAsrSessionState(sessionId)
      if (state.latest_revision) applyPublication(state.latest_revision)
      if (state.latest_job?.state === 'failed') {
        showDurableFailure({
          session_id: sessionId,
          job_id: state.latest_job.id,
          chunk_id: state.latest_job.chunk_id,
          error_code: state.latest_job.error_code ?? 'transcription_failed',
          error_summary: state.latest_job.error_summary ?? '本地 ASR 失败',
          retry_available: true,
        })
      }
    } catch {
      // The next committed event can still reconcile this view.
    }
  }

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
    if (!desktopRuntime) return
    let disposed = false
    const restoredSessionId = window.sessionStorage.getItem(ACTIVE_LIVE_SESSION_KEY)
    if (restoredSessionId) {
      displaySessionIdRef.current = restoredSessionId
    }
    void Promise.all([
      listen<CommittedPublication>('asr-publication-committed', (event) => {
        applyPublication(event.payload)
      }),
      listen<DurableAsrFailurePayload>('asr-job-failed', (event) => {
        showDurableFailure(event.payload)
      }),
    ]).then((unlisteners) => {
      const unlisten = () => unlisteners.forEach((stop) => stop())
      if (disposed) {
        unlisten()
      } else {
        publicationUnlistenRef.current = unlisten
        if (restoredSessionId) void reconcileSession(restoredSessionId)
      }
    })
    return () => {
      disposed = true
      publicationUnlistenRef.current?.()
      publicationUnlistenRef.current = null
    }
  }, [desktopRuntime])

  useEffect(() => {
    captureStateRef.current = captureState
  }, [captureState])

  useEffect(() => {
    if (captureState !== 'recording') return

    const startedAt = performance.now()
    const updateElapsed = () => {
      setElapsedMs(elapsedAccumulatedRef.current + performance.now() - startedAt)
    }
    updateElapsed()
    const intervalId = window.setInterval(updateElapsed, ELAPSED_REFRESH_INTERVAL_MS)

    return () => {
      window.clearInterval(intervalId)
      elapsedAccumulatedRef.current += Math.max(0, performance.now() - startedAt)
    }
  }, [captureState])

  useLayoutEffect(() => {
    captureInputRef.current = {
      mode: microphone.captureMode,
      microphoneUid: microphone.selectedUid,
      availability: microphone.availability,
    }
  }, [microphone.availability, microphone.captureMode, microphone.selectedUid])

  useEffect(() => {
    microphone.setLocked(
      lifecycleBusy || captureState === 'recording' || captureState === 'paused',
    )
  }, [captureState, lifecycleBusy])

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
    elapsedAccumulatedRef.current = 0
    setElapsedMs(0)
    setSegments([])
    setNotes([])
    setShowPolished(false)
    setPolishedTexts({})
    setQuickInputActive(false)
    setErrorMessage(null)
    displaySessionIdRef.current = null
    appliedRevisionNumberRef.current = 0
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
    let captureInput = captureInputRef.current
    if (desktopRuntime) {
      try {
        captureInput = await microphone.resolveStartInput()
      } catch {
        startInFlightRef.current = false
        setLifecycleBusyState(false)
        setErrorMessage(RECORDING_CONFIG_FAILURE_NOTICE)
        onNotice(RECORDING_CONFIG_FAILURE_NOTICE)
        return
      }
    }
    if (
      desktopRuntime
      && captureInput.mode !== 'system-only'
      && captureInput.availability !== 'ready'
    ) {
      const message = microphoneStartBlockMessage(captureInput.availability)
      startInFlightRef.current = false
      setLifecycleBusyState(false)
      setErrorMessage(message)
      onNotice(message)
      return
    }

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
        displaySessionIdRef.current = session.id
        window.sessionStorage.setItem(ACTIVE_LIVE_SESSION_KEY, session.id)

        const unlistenError = await listen<StreamingErrorPayload>('asr-live-error', (event) => {
          beginFailedAttempt(attemptId, event.payload.message)
        })
        unlistenRef.current = () => {
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

        await startStreamingCapture(
          session.id,
          captureInput.mode,
          captureInput.mode === 'system-only' ? null : captureInput.microphoneUid,
        )
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
      onNotice(BROWSER_DEMO_STARTED_NOTICE)
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
    setQuickInputActive(false)

    let streamStopped = true
    try {
      await stopStreamingCapture()
    } catch (error) {
      streamStopped = !desktopRuntime
      if (desktopRuntime) {
        setCaptureState('error')
        setErrorMessage('当前会话停止失败，请检查本地数据目录。')
        onNotice('当前会话停止失败，请检查本地数据目录。')
      }
    }
    if (!streamStopped) {
      if (lifecycleGenerationRef.current === lifecycleGeneration) {
        setLifecycleBusyState(false)
      }
      return
    }
    setCaptureState('stopped')
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
    onNotice(desktopRuntime ? STOPPED_NOTICE : BROWSER_DEMO_STOPPED_NOTICE)
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
        onNotice('示例转写仅执行规则清理，不会调用本地 LLM。')
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
    setEditingSpeakerId(null)
    setSpeakerDraft('')
  }

  const isStarting = captureState === 'idle' && lifecycleBusy
  const microphoneRequired = microphone.captureMode !== 'system-only'
  const microphoneBlocksStart = desktopRuntime
    && microphoneRequired
    && microphone.availability !== 'ready'
  const showReadyAction = captureState === 'idle' || captureState === 'stopped'
  const rootStateClassName = [
    'live-capture',
    `live-capture--${captureState}`,
    isStarting ? 'live-capture--starting' : '',
    errorMessage ? 'live-capture--error' : '',
  ].filter(Boolean).join(' ')

  const statusTitle = desktopRuntime
    ? isStarting
      ? '正在启动'
      : captureState === 'error'
        ? '采集失败'
      : captureState === 'idle'
      ? errorMessage
        ? '采集未就绪'
        : '待检测'
      : captureState === 'recording'
        ? '正在记录'
        : captureState === 'paused'
          ? '已暂停'
          : '记录已停止'
    : captureState === 'recording'
      ? '正在播放示例转写'
      : captureState === 'paused'
        ? '示例转写已暂停'
        : captureState === 'stopped'
          ? '示例转写已结束'
          : '示例转写待播放'

  const statusSubtitle = desktopRuntime
    ? errorMessage
      ? '请查看下方错误详情后重试。'
      : isStarting
        ? startupStatusSubtitle(microphone.captureMode)
      : captureState === 'idle'
        ? '点击「开始记录」或按 ⌘R 启动；失败时不会展示任何演示转写。'
        : '桌面实时采集失败时会显示错误，不会回退演示数据。'
    : captureState === 'recording'
      ? '示例内容将按时间顺序显示。'
      : captureState === 'paused'
        ? '继续播放可恢复示例转写。'
        : '播放示例转写，预览实时记录界面。'

  return (
    <main className={rootStateClassName}>
      <header className="live-capture__bar">
        <div className="live-capture__status">
          <span className={`recorder__pulse recorder__pulse--${isStarting ? 'recording' : captureState}`} aria-hidden="true" />
          <div>
            <strong>{statusTitle}</strong>
            <small>{statusSubtitle}</small>
          </div>
        </div>
        {desktopRuntime && (microphoneRequired ? (
          <MicrophoneDeviceSelector
            state={microphone}
            compact
            locked={lifecycleBusy || captureState === 'recording' || captureState === 'paused'}
            onSelect={(uid) => void microphone.select(uid)}
            onRefresh={() => void microphone.refresh()}
            onProbe={() => void microphone.probe()}
          />
        ) : (
          <div className="live-capture__asr"><Monitor size={14} />系统音频</div>
        ))}
        <div className="live-capture__asr">
          {desktopRuntime ? '本地 ASR · 待接通' : '示例转写'}
        </div>
        <div className="live-capture__actions">
          {(captureState === 'recording' || captureState === 'paused') && (
            <output className="live-capture__elapsed" aria-label="当前时长">
              {formatTime(elapsedMs)}
            </output>
          )}
          {captureState === 'recording' && (
            <>
              <button className="button" onClick={togglePause} disabled={lifecycleBusy}>
                <CirclePause size={17} />暂停
              </button>
              <button className="button live-capture__note-action" disabled={lifecycleBusy} onClick={() => {
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
                  <button className="text-button" onClick={polishAll} disabled={polishing} title="仅本页预览，不保存为修订或用于导出">
                    <Sparkles size={14} />{polishing ? '预览生成中...' : '临时润色预览'}
                  </button>
                )}
                <button className="text-button" onClick={copyAll}>
                  <Copy size={14} />复制原始转写
                </button>
              </>
            )}
          </div>

          {showPolished && <p className="live-capture__preview-note">当前润色只在本页预览；复制、搜索与导出仍使用原始转写。</p>}

          {showReadyAction && (
            <div className="live-capture__ready">
              <div className="live-capture__ready-copy">
                <strong>{captureState === 'stopped' ? '可以开始下一次记录' : statusTitle}</strong>
                <small>
                  {desktopRuntime
                    ? captureState === 'stopped'
                      ? '当前内容仍保留在页面中，可随时开始新的记录。'
                      : '确认输入设备和模式后开始记录。'
                    : captureState === 'stopped'
                      ? '可重新播放同一段示例转写。'
                      : '播放后可预览实时转写与笔记布局。'}
                </small>
              </div>
              <div className="live-capture__ready-actions">
                <button className="button button--primary" onClick={startCapture} disabled={lifecycleBusy || microphoneBlocksStart}>
                  <span className="recorder__pulse recorder__pulse--recording" />
                  {isStarting
                    ? '正在启动'
                    : desktopRuntime
                      ? captureState === 'stopped' ? '开始新记录' : '开始记录'
                      : captureState === 'stopped' ? '重新播放示例转写' : '播放示例转写'}
                </button>
              </div>
            </div>
          )}

          {segments.length === 0 && captureState === 'idle' && (
            <div className="live-capture__empty">
              <div className="live-capture__empty-icon">
                {microphoneRequired ? <Mic size={28} /> : <Monitor size={28} />}
              </div>
              <strong>
                {desktopRuntime
                  ? isStarting ? '正在启动' : errorMessage ? '采集未就绪' : '待检测'
                  : '示例转写待播放'}
              </strong>
              <p>
                {desktopRuntime
                  ? isStarting
                    ? startupConnectionMessage(microphone.captureMode)
                    : '点击「开始记录」或按 ⌘R 启动；失败时不会展示任何演示转写。'
                  : '播放一段示例内容，预览实时转写与笔记布局。'}
              </p>
              {desktopRuntime && (isStarting || errorMessage) && (
                <ul className="live-capture__status-list">
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    {microphoneRequired ? <Mic size={14} /> : <Monitor size={14} />}
                    <span className="live-capture__status-label">{microphoneRequired ? '麦克风权限' : '系统录音权限'}</span>
                    <span className="live-capture__status-value">未检测</span>
                    <span className="live-capture__status-hint">点击「开始记录」自动请求</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <Monitor size={14} />
                    <span className="live-capture__status-label">{microphoneRequired ? '设备状态' : '系统音频'}</span>
                    <span className="live-capture__status-value">待接通</span>
                    <span className="live-capture__status-hint">{microphoneRequired ? '在偏好设置中检查输入设备' : '需要录屏与系统录音权限'}</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <Cpu size={14} />
                    <span className="live-capture__status-label">Provider</span>
                    <span className="live-capture__status-value">转写状态待确认</span>
                    <span className="live-capture__status-hint">转写由已提交的 Job 状态确认，可在时间线查看</span>
                  </li>
                  <li className="live-capture__status-item live-capture__status-item--pending">
                    <HardDrive size={14} />
                    <span className="live-capture__status-label">保存状态</span>
                    <span className="live-capture__status-value">需等待持久化确认</span>
                    <span className="live-capture__status-hint">开始记录后自动创建 Catalog 会话</span>
                  </li>
                </ul>
              )}
            </div>
          )}
          {segments.length === 0 && captureState === 'recording' && (
            <div className="empty-state">
              <strong className="live-capture__listening">
                {desktopRuntime && (microphoneRequired ? <Mic size={18} aria-hidden="true" /> : <Monitor size={18} aria-hidden="true" />)}
                {desktopRuntime ? microphoneRequired ? '正在监听...' : '正在监听系统音频...' : '正在准备示例转写...'}
              </strong>
              <p>{desktopRuntime ? '检测到语音后将自动开始转写' : '示例内容即将显示'}</p>
            </div>
          )}
          {errorMessage && <p className="live-capture__error" role="alert">{errorMessage}</p>}

          <div className="live-capture__segments">
            {segments.map((seg) => (
              <article key={seg.id} className="live-segment">
                <div className="live-segment__header">
                  {editingSpeakerId === seg.id ? (
                    <div className="live-segment__speaker-editor">
                      <input
                        aria-label="说话人名称"
                        value={speakerDraft}
                        onChange={(event) => setSpeakerDraft(event.target.value)}
                        autoFocus
                      />
                      <button className="text-button" disabled={!speakerDraft.trim()} onClick={() => renameSpeaker(seg.id, speakerDraft.trim())}>确认</button>
                      <button className="text-button" onClick={() => { setEditingSpeakerId(null); setSpeakerDraft('') }}>取消</button>
                    </div>
                  ) : seg.speaker.source === 'unknown' || seg.speaker.source === 'dictionary' ? (
                    <button
                      className={`live-segment__speaker live-segment__speaker--${seg.speaker.source}`}
                      title={seg.speaker.source === 'unknown' ? '点击重命名说话人' : '来自词典（点击确认）'}
                      onClick={() => {
                        setEditingSpeakerId(seg.id)
                        setSpeakerDraft(seg.speaker.source === 'dictionary' ? seg.speaker.label.replace('可能是', '').replace('？', '') : '')
                      }}
                    >
                      {seg.speaker.label}
                    </button>
                  ) : (
                    <span
                      className={`live-segment__speaker live-segment__speaker--${seg.speaker.source}`}
                      title={seg.speaker.source === 'voiceprint' ? '来自声纹库' : '手动标注'}
                    >
                      {seg.speaker.label}
                    </span>
                  )}
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

function microphoneStartBlockMessage(availability: MicrophoneAvailability) {
  switch (availability) {
    case 'loading': return '正在检测麦克风，请稍候。'
    case 'empty': return '未检测到可用麦克风。'
    case 'disconnected': return '已选择的麦克风已断开，请重新连接或选择其他设备。'
    case 'error': return '麦克风设备读取失败，请刷新后重试。'
    case 'unselected': return '开始记录前请先选择收音麦克风。'
    case 'ready': return ''
  }
}

function sourceLabel(source: string) {
  if (source === 'microphone') return '麦克风'
  if (source === 'system_audio') return '系统音频'
  if (source === 'imported') return '导入音频'
  return '未知来源'
}

function startupStatusSubtitle(mode: CaptureMode) {
  if (mode === 'mic-only') return '正在检查麦克风和保存状态。'
  if (mode === 'system-only') return '正在检查系统音频和保存状态。'
  return '正在检查麦克风、系统音频和保存状态。'
}

function startupConnectionMessage(mode: CaptureMode) {
  if (mode === 'mic-only') return '正在请求麦克风权限并连接所选设备。'
  if (mode === 'system-only') return '正在检查录屏与系统录音权限并连接系统音频。'
  return '正在请求权限并连接麦克风与系统音频。'
}
