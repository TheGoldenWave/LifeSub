import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useLayoutEffect, useState } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const eventListeners = new Map<string, Array<(payload: unknown) => void>>()

const createCapture = vi.fn()
const transitionCapture = vi.fn()
const startStreamingCapture = vi.fn()
const stopStreamingCapture = vi.fn()
const pauseStreamingCapture = vi.fn()
const resumeStreamingCapture = vi.fn()
const llmPolish = vi.fn()
const registerQuickInputHotkey = vi.fn()
const getAsrSessionState = vi.fn()
const isTauriRuntime = vi.fn()
const refreshMicrophones = vi.fn()
const selectMicrophone = vi.fn()
const resolveMicrophoneStartInput = vi.fn()
const setMicrophoneLocked = vi.fn()
const microphoneState = {
  phase: 'ready' as const,
  availability: 'ready' as const,
  devices: [{
    uid: 'mic-uid-1',
    name: 'MacBook Pro 麦克风',
    isDefault: true,
    isConnected: true,
    transport: 'built_in' as const,
  }],
  selectedUid: 'mic-uid-1' as string | null,
  selectedName: 'MacBook Pro 麦克风' as string | null,
  captureMode: 'smart' as const,
  error: null as string | null,
  refreshing: false,
  saving: false,
  locked: false,
}

const loadNotes = vi.fn()
const createNoteAdapter = vi.fn()
const deleteNoteAdapter = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (eventName: string, callback: (event: { payload: unknown }) => void) => {
    const listeners = eventListeners.get(eventName) ?? []
    const wrapped = (payload: unknown) => callback({ payload })
    listeners.push(wrapped)
    eventListeners.set(eventName, listeners)
    return () => {
      const next = (eventListeners.get(eventName) ?? []).filter((listener) => listener !== wrapped)
      eventListeners.set(eventName, next)
    }
  }),
}))

vi.mock('../services/lifesub', () => ({
  createCapture,
  transitionCapture,
  startStreamingCapture,
  stopStreamingCapture,
  pauseStreamingCapture,
  resumeStreamingCapture,
  llmPolish,
  registerQuickInputHotkey,
  getAsrSessionState,
  isTauriRuntime,
}))

vi.mock('../data/adapter', () => ({
  loadNotes,
  createNoteAdapter,
  deleteNoteAdapter,
}))

vi.mock('../microphoneDevices', () => ({
  useMicrophoneDevices: () => ({
    ...microphoneState,
    refresh: refreshMicrophones,
    select: selectMicrophone,
    resolveStartInput: resolveMicrophoneStartInput,
    synchronizeConfig: vi.fn(),
    setLocked: setMicrophoneLocked,
  }),
}))

function emitEvent(eventName: string, payload: unknown) {
  for (const listener of eventListeners.get(eventName) ?? []) {
    listener(payload)
  }
}

describe('LiveCapture safety flows', () => {
  beforeEach(() => {
    eventListeners.clear()
    createCapture.mockReset()
    transitionCapture.mockReset()
    startStreamingCapture.mockReset()
    stopStreamingCapture.mockReset()
    pauseStreamingCapture.mockReset()
    resumeStreamingCapture.mockReset()
    llmPolish.mockReset()
    registerQuickInputHotkey.mockReset()
    getAsrSessionState.mockReset()
    isTauriRuntime.mockReset()
    loadNotes.mockReset()
    createNoteAdapter.mockReset()
    deleteNoteAdapter.mockReset()
    refreshMicrophones.mockReset()
    selectMicrophone.mockReset()
    resolveMicrophoneStartInput.mockReset()
    setMicrophoneLocked.mockReset()

    Object.assign(microphoneState, {
      phase: 'ready',
      availability: 'ready',
      devices: [{
        uid: 'mic-uid-1',
        name: 'MacBook Pro 麦克风',
        isDefault: true,
        isConnected: true,
        transport: 'built_in',
      }],
      selectedUid: 'mic-uid-1',
      selectedName: 'MacBook Pro 麦克风',
      captureMode: 'smart',
      error: null,
      refreshing: false,
      saving: false,
      locked: false,
    })
    resolveMicrophoneStartInput.mockImplementation(async () => ({
      mode: microphoneState.captureMode,
      microphoneUid: microphoneState.selectedUid,
      availability: microphoneState.availability,
    }))

    isTauriRuntime.mockReturnValue(true)
    loadNotes.mockResolvedValue([])
    createCapture.mockResolvedValue({
      id: 'session-1',
      title: '实时记录',
      state: 'idle',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: null,
    })
    transitionCapture.mockImplementation(async (session, target) => ({
      ...session,
      state: target,
      ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
    }))
    startStreamingCapture.mockResolvedValue(undefined)
    stopStreamingCapture.mockResolvedValue(undefined)
    registerQuickInputHotkey.mockResolvedValue(undefined)
    getAsrSessionState.mockResolvedValue({ latest_revision: null, latest_job: null })
    window.sessionStorage.clear()
    createNoteAdapter.mockImplementation(async (sessionId, content, timestampMs, tag, segmentId) => ({
      id: 'note-1',
      content,
      timestampMs,
      tag,
      segmentId,
      createdAt: '2026-08-19T00:00:05Z',
      sessionId,
    }))
  })

  it('starts desktop capture with the explicitly selected microphone UID', async () => {
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    expect(startStreamingCapture).toHaveBeenCalledWith('session-1', 'smart', 'mic-uid-1')
  })

  it('keeps inactive primary actions in the ready area and active controls in the top bar', async () => {
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    const { container } = render(<LiveCapture onNotice={vi.fn()} />)
    const root = container.querySelector('.live-capture')
    const topBar = container.querySelector('.live-capture__bar')
    const readyArea = container.querySelector('.live-capture__ready')

    expect(root).toHaveClass('live-capture--idle')
    expect(within(readyArea as HTMLElement).getByRole('button', { name: '开始记录' })).toBeInTheDocument()
    expect(within(topBar as HTMLElement).queryByRole('button', { name: '开始记录' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '开始记录' }))

    expect(root).toHaveClass('live-capture--recording')
    expect(within(topBar as HTMLElement).getByRole('button', { name: '暂停' })).toBeInTheDocument()
    expect(topBar?.querySelector('.live-capture__elapsed')).toBeInTheDocument()
    expect(within(topBar as HTMLElement).getByRole('button', { name: '新笔记' })).toHaveClass('live-capture__note-action')

    await user.click(screen.getByRole('button', { name: '停止' }))

    expect(root).toHaveClass('live-capture--stopped')
    const stoppedReadyArea = container.querySelector('.live-capture__ready')
    expect(within(stoppedReadyArea as HTMLElement).getByRole('button', { name: '开始新记录' })).toBeInTheDocument()
    expect(within(topBar as HTMLElement).queryByRole('button', { name: '开始新记录' })).not.toBeInTheDocument()
  })

  it('accumulates elapsed recording time while excluding paused time', async () => {
    vi.useFakeTimers()
    isTauriRuntime.mockReturnValue(false)
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: '播放示例转写' }))
    expect(screen.getByLabelText('当前时长')).toHaveTextContent('00:00')

    await act(() => vi.advanceTimersByTimeAsync(2100))
    expect(screen.getByLabelText('当前时长')).toHaveTextContent('00:02')

    fireEvent.click(screen.getByRole('button', { name: '暂停' }))
    await act(() => vi.advanceTimersByTimeAsync(2000))
    expect(screen.getByLabelText('当前时长')).toHaveTextContent('00:02')

    fireEvent.click(screen.getByRole('button', { name: '继续' }))
    await act(() => vi.advanceTimersByTimeAsync(1100))
    expect(screen.getByLabelText('当前时长')).toHaveTextContent('00:03')

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '停止' }))
      await Promise.resolve()
    })
    fireEvent.click(screen.getByRole('button', { name: '重新播放示例转写' }))
    expect(screen.getByLabelText('当前时长')).toHaveTextContent('00:00')
    vi.useRealTimers()
  })

  it('blocks microphone capture before creating a session when no explicit device is available', async () => {
    Object.assign(microphoneState, {
      availability: 'unselected',
      selectedUid: null,
      selectedName: null,
    })
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={vi.fn()} />)

    expect(screen.getByRole('button', { name: '开始记录' })).toBeDisabled()
    expect(createCapture).not.toHaveBeenCalled()
  })

  it('allows system-only capture when no microphone exists', async () => {
    Object.assign(microphoneState, {
      availability: 'empty',
      devices: [],
      selectedUid: null,
      selectedName: null,
      captureMode: 'system-only',
    })
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={vi.fn()} />)
    expect(screen.queryByRole('combobox', { name: '收音麦克风' })).not.toBeInTheDocument()
    expect(screen.getAllByText('系统音频').length).toBeGreaterThan(0)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    expect(startStreamingCapture).toHaveBeenCalledWith('session-1', 'system-only', null)
  })

  it('uses the latest capture mode when settings change immediately before start', async () => {
    Object.assign(microphoneState, { captureMode: 'mic-only' })
    const { LiveCapture } = await import('./LiveCapture')

    function ImmediateModeChangeHarness() {
      const [mode, setMode] = useState<'mic-only' | 'system-only'>('mic-only')
      Object.assign(microphoneState, { captureMode: mode })
      useLayoutEffect(() => {
        if (mode === 'mic-only') {
          setMode('system-only')
          return
        }
        document.querySelector<HTMLButtonElement>('button.button--primary')?.click()
      }, [mode])
      return <LiveCapture onNotice={vi.fn()} />
    }

    render(<ImmediateModeChangeHarness />)

    await waitFor(() => {
      expect(startStreamingCapture).toHaveBeenCalledWith('session-1', 'system-only', null)
    })
  })

  it('describes only the requested source while mic-only startup is pending', async () => {
    Object.assign(microphoneState, { captureMode: 'mic-only' })
    startStreamingCapture.mockImplementation(() => new Promise<void>(() => undefined))
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    expect(screen.getByText('正在检查麦克风和保存状态。')).toBeInTheDocument()
    expect(screen.getByText('正在请求麦克风权限并连接所选设备。')).toBeInTheDocument()
    expect(screen.queryByText(/连接麦克风与系统音频/)).not.toBeInTheDocument()
    expect(screen.queryByText('本地 ASR 未接通')).not.toBeInTheDocument()
    expect(screen.getByText('转写状态待确认')).toBeInTheDocument()
  })

  it('shows a visible desktop error instead of falling back to demo data', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '桌面实时采集未接通，已阻止演示数据回退。',
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('桌面实时采集未接通，已阻止演示数据回退。')
    expect(screen.getAllByText('采集未就绪').length).toBeGreaterThan(0)
    expect(screen.getByText('请查看下方错误详情后重试。')).toBeInTheDocument()
    expect(screen.getAllByText('桌面实时采集未接通，已阻止演示数据回退。')).toHaveLength(1)
    expect(document.querySelector('.live-capture')).toHaveClass('live-capture--error')
    expect(screen.queryByText('我们今天先确认首版范围，重点是把基础闭环真正跑起来。')).not.toBeInTheDocument()
    expect(onNotice).toHaveBeenCalledWith('桌面实时采集未接通，已阻止演示数据回退。')
  })

  it('labels the error state without repeating its specific message in the subtitle', async () => {
    stopStreamingCapture.mockRejectedValueOnce(new Error('stop failed'))
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '停止' }))

    expect(await screen.findByText('采集失败')).toBeInTheDocument()
    expect(screen.getByText('请查看下方错误详情后重试。')).toBeInTheDocument()
    expect(screen.getAllByText('当前会话停止失败，请检查本地数据目录。')).toHaveLength(1)
    expect(document.querySelector('.live-capture')).toHaveClass('live-capture--error')
  })

  it('shows one disabled starting control and visible startup status while capture startup is pending', async () => {
    startStreamingCapture.mockImplementation(() => new Promise<void>(() => undefined))
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    const startButtons = screen.getAllByRole('button', { name: '正在启动' })
    expect(startButtons).toHaveLength(1)
    expect(startButtons[0]).toBeDisabled()
    expect(screen.getAllByText('正在启动')).toHaveLength(4)
    expect(screen.queryByText('待检测')).not.toBeInTheDocument()
  })

  it('keeps streaming listeners alive across captureState changes', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '暂停' }))

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '暂停后仍应接收错误事件',
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('暂停后仍应接收错误事件')
    expect(onNotice).toHaveBeenCalledWith('暂停后仍应接收错误事件')
  })

  it('renders only committed publication events and replaces duplicate revision delivery', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-live-segment', {
      id: 'unpersisted',
      startMs: 200,
      speaker: { id: 'unknown', label: '未知', source: 'unknown', voiceprintId: null },
      text: '不应显示的瞬时文本',
      completed: true,
    })
    expect(screen.queryByText('不应显示的瞬时文本')).not.toBeInTheDocument()

    const publication = {
      revision_id: 'revision-1',
      session_id: 'session-1',
      revision_number: 1,
      provider: 'sense_voice',
      created_at: '2026-08-25T06:00:00Z',
      segments: [{
        segment_id: 'segment-committed',
        chunk_id: 'chunk-1',
        capture_source_id: 'source-mic',
        start_ms: 486,
        end_ms: 6208,
        source: 'microphone',
        text: '已经持久化的原话',
      }],
    }
    emitEvent('asr-publication-committed', publication)
    emitEvent('asr-publication-committed', publication)

    expect(await screen.findByText('已经持久化的原话')).toBeInTheDocument()
    expect(screen.getAllByText('已经持久化的原话')).toHaveLength(1)
  })

  it('keeps the committed publication listener after capture stops', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '停止' }))

    emitEvent('asr-publication-committed', {
      revision_id: 'revision-after-stop',
      session_id: 'session-1',
      revision_number: 1,
      provider: 'sense_voice',
      created_at: '2026-08-25T06:00:00Z',
      segments: [{
        segment_id: 'segment-after-stop', chunk_id: 'chunk-2', capture_source_id: 'source-system',
        start_ms: 900, end_ms: 1900, source: 'system_audio', text: '停止后完成的持久化原话',
      }],
    })

    expect(await screen.findByText('停止后完成的持久化原话')).toBeInTheDocument()
  })

  it('recovers a missed committed revision after remount and ignores older replay', async () => {
    window.sessionStorage.setItem('lifesub.activeLiveSessionId', 'session-reconnect')
    getAsrSessionState.mockResolvedValue({
      latest_revision: {
        revision_id: 'revision-2', session_id: 'session-reconnect', revision_number: 2,
        provider: 'sense_voice', created_at: '2026-08-25T06:00:00Z',
        segments: [{
          segment_id: 'segment-new', chunk_id: 'chunk-new', capture_source_id: 'source-mic',
          start_ms: 1000, end_ms: 2000, source: 'microphone', text: '重连后恢复的持久化原话',
        }],
      },
      latest_job: { id: 'job-2', chunk_id: 'chunk-new', state: 'succeeded', error_code: null, error_summary: null },
    })
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={vi.fn()} />)
    expect(await screen.findByText('重连后恢复的持久化原话')).toBeInTheDocument()

    emitEvent('asr-publication-committed', {
      revision_id: 'revision-1', session_id: 'session-reconnect', revision_number: 1,
      provider: 'sense_voice', created_at: '2026-08-25T05:59:00Z', segments: [{
        segment_id: 'segment-old', chunk_id: 'chunk-old', capture_source_id: 'source-mic',
        start_ms: 0, end_ms: 500, source: 'microphone', text: '不应回退到旧 revision',
      }],
    })

    expect(screen.queryByText('不应回退到旧 revision')).not.toBeInTheDocument()
    expect(screen.getAllByText('重连后恢复的持久化原话')).toHaveLength(1)
  })

  it('shows a durable ASR job failure without adding transcript text', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    emitEvent('asr-job-failed', {
      session_id: 'session-1', job_id: 'job-failed', chunk_id: 'chunk-failed',
      error_code: 'transcription_failed', error_summary: '本地模型推理失败', retry_available: true,
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('本地模型推理失败')
    expect(screen.queryByText('我们今天先确认首版范围')).not.toBeInTheDocument()
  })

  it('offers a second recording entry and never claims capture was saved', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)

    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '停止' }))

    expect(await screen.findByRole('button', { name: '开始新记录' })).toBeInTheDocument()
    expect(onNotice).toHaveBeenCalled()
    expect(onNotice.mock.calls.some(([message]) => String(message).includes('已保存'))).toBe(false)

    await user.click(screen.getByRole('button', { name: '开始新记录' }))
    expect(createCapture).toHaveBeenCalledTimes(2)
    expect(startStreamingCapture).toHaveBeenCalledTimes(2)
  })

  it('does not overwrite a failure state when the backend start promise resolves late', async () => {
    const onNotice = vi.fn()
    let resolveStart: (() => void) | undefined
    startStreamingCapture.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveStart = () => resolve()
        }),
    )

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '先收到错误，再等后台返回',
    })
    if (resolveStart) resolveStart()

    expect(await screen.findByRole('alert')).toHaveTextContent('先收到错误，再等后台返回')
    await waitFor(() => expect(screen.queryByText('正在记录')).not.toBeInTheDocument())
    expect(
      transitionCapture.mock.calls.filter(([, target]) => target === 'recording'),
    ).toHaveLength(1)
    expect(
      transitionCapture.mock.calls.filter(([, target]) => target === 'stopped'),
    ).toHaveLength(1)
  })

  it('cleans up the session if an error arrives while recording transition is still pending', async () => {
    const onNotice = vi.fn()
    let resolveRecording: ((session: {
      id: string
      title: string
      state: 'recording'
      started_at: string
      ended_at: null
    }) => void) | undefined

    transitionCapture.mockImplementation((session, target) => {
      if (target === 'recording') {
        return new Promise((resolve) => {
          resolveRecording = resolve
        })
      }
      return Promise.resolve({
        ...session,
        state: target,
        ended_at: '2026-08-19T00:05:00Z',
      })
    })

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '录制启动时失败',
    })

    if (resolveRecording) resolveRecording({
      id: 'session-1',
      title: '实时记录',
      state: 'recording',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: null,
    })

    await waitFor(() =>
      expect(transitionCapture).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'session-1', state: 'recording' }),
        'stopped',
      ),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('录制启动时失败')
    expect(onNotice).toHaveBeenCalledWith('录制启动时失败')
  })

  it('starts capture on Command+R but ignores editable and dialog contexts', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={onNotice} />)

    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true }))
    expect(startStreamingCapture).not.toHaveBeenCalled()

    const dialog = document.createElement('div')
    dialog.setAttribute('role', 'dialog')
    const dialogButton = document.createElement('button')
    dialog.appendChild(dialogButton)
    document.body.appendChild(dialog)
    dialogButton.focus()
    dialogButton.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true }))
    expect(startStreamingCapture).not.toHaveBeenCalled()

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true }))
    await waitFor(() => expect(startStreamingCapture).toHaveBeenCalledTimes(1))
  })

  it('ignores a second Command+R while start is already in flight', async () => {
    let resolveStart: (() => void) | undefined
    startStreamingCapture.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveStart = resolve
        }),
    )

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={onNotice} />)

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true }))
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true }))

    await waitFor(() => expect(createCapture).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(startStreamingCapture).toHaveBeenCalledTimes(1))

    if (resolveStart) resolveStart()
  })

  it('persists notes against the active session using a monotonic relative timestamp', async () => {
    const onNotice = vi.fn()
    const nowSpy = vi.spyOn(performance, 'now').mockReturnValue(1000)

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)

    await user.click(screen.getByRole('button', { name: '开始记录' }))
    nowSpy.mockReturnValue(4600)
    await user.click(screen.getAllByRole('button', { name: '新笔记' })[1])
    await user.type(screen.getByPlaceholderText('输入笔记内容...'), '确认安全边界')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() =>
      expect(createNoteAdapter).toHaveBeenCalledWith(
        'session-1',
        '确认安全边界',
        3600,
        '备忘',
        null,
      ),
    )
    expect(screen.getByText('确认安全边界')).toBeInTheDocument()

    nowSpy.mockRestore()
  })

  it('transitions the desktop session to recording before it can stop', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)

    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '停止' }))

    expect(transitionCapture).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ id: 'session-1', state: 'idle' }),
      'recording',
    )
    expect(transitionCapture).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ id: 'session-1', state: 'recording' }),
      'stopped',
    )
  })

  it('rolls back pause UI when catalog transition to paused fails', async () => {
    const onNotice = vi.fn()
    transitionCapture.mockImplementation(async (session, target) => {
      if (target === 'paused') {
        throw new Error('catalog unavailable')
      }
      return {
        ...session,
        state: target,
        ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
      }
    })

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '暂停' }))

    await waitFor(() => expect(onNotice).toHaveBeenCalledWith('暂停失败，当前会话仍保持录制中。'))
    expect(screen.getByText('正在记录')).toBeInTheDocument()
  })

  it('disables stop while pause transition is still in flight', async () => {
    let resolvePause: ((session: {
      id: string
      title: string
      state: 'paused'
      started_at: string
      ended_at: null
    }) => void) | undefined
    transitionCapture.mockImplementation((session, target) => {
      if (target === 'paused') {
        return new Promise((resolve) => {
          resolvePause = resolve
        })
      }
      return Promise.resolve({
        ...session,
        state: target,
        ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
      })
    })

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '暂停' }))

    expect(screen.getByRole('button', { name: '停止' })).toBeDisabled()

    if (resolvePause) resolvePause({
      id: 'session-1',
      title: '实时记录',
      state: 'paused',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: null,
    })

    await waitFor(() => expect(screen.getByText('已暂停')).toBeInTheDocument())
  })

  it('does not revive the session if an error arrives during delayed pause transition', async () => {
    let resolvePause: ((session: {
      id: string
      title: string
      state: 'paused'
      started_at: string
      ended_at: null
    }) => void) | undefined
    transitionCapture.mockImplementation((session, target) => {
      if (target === 'paused') {
        return new Promise((resolve) => {
          resolvePause = resolve
        })
      }
      return Promise.resolve({
        ...session,
        state: target,
        ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
      })
    })

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '暂停' }))

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '暂停中收到后端错误',
    })
    if (resolvePause) resolvePause({
      id: 'session-1',
      title: '实时记录',
      state: 'paused',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: null,
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('暂停中收到后端错误')
    await waitFor(() => expect(screen.queryByText('已暂停')).not.toBeInTheDocument())
    expect(
      transitionCapture.mock.calls.filter(
        ([session, target]) => session.state === 'paused' && target === 'recording',
      ),
    ).toHaveLength(0)
  })

  it('does not revive the session if an error arrives during delayed resume transition', async () => {
    let resolveResume: ((session: {
      id: string
      title: string
      state: 'recording'
      started_at: string
      ended_at: null
    }) => void) | undefined
    transitionCapture.mockImplementation((session, target) => {
      if (target === 'paused') {
        return Promise.resolve({
          ...session,
          state: 'paused',
          ended_at: null,
        })
      }
      if (target === 'recording' && session.state === 'paused') {
        return new Promise((resolve) => {
          resolveResume = resolve
        })
      }
      return Promise.resolve({
        ...session,
        state: target,
        ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
      })
    })

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '暂停' }))
    await waitFor(() => expect(screen.getByText('已暂停')).toBeInTheDocument())
    await user.click(screen.getByRole('button', { name: '继续' }))
    const pauseRollbackCallsBeforeError = transitionCapture.mock.calls.filter(
      ([session, target]) => session.state === 'recording' && target === 'paused',
    ).length

    emitEvent('asr-live-error', {
      code: 'streaming_unavailable',
      message: '继续中收到后端错误',
    })
    if (resolveResume) resolveResume({
      id: 'session-1',
      title: '实时记录',
      state: 'recording',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: null,
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('继续中收到后端错误')
    await waitFor(() => expect(screen.queryByText('正在记录')).not.toBeInTheDocument())
    expect(
      transitionCapture.mock.calls.filter(
        ([session, target]) => session.state === 'recording' && target === 'paused',
      ),
    ).toHaveLength(pauseRollbackCallsBeforeError)
  })

  it('prevents restart until a delayed stop transition finishes', async () => {
    let resolveStop: ((session: {
      id: string
      title: string
      state: 'stopped'
      started_at: string
      ended_at: string
    }) => void) | undefined
    transitionCapture.mockImplementation((session, target) => {
      if (target === 'stopped') {
        return new Promise((resolve) => {
          resolveStop = resolve
        })
      }
      return Promise.resolve({
        ...session,
        state: target,
        ended_at: target === 'stopped' ? '2026-08-19T00:05:00Z' : null,
      })
    })

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getByRole('button', { name: '停止' }))

    const restartButton = await screen.findByRole('button', { name: '开始新记录' })
    expect(restartButton).toBeDisabled()
    await user.click(restartButton)
    expect(createCapture).toHaveBeenCalledTimes(1)

    if (resolveStop) resolveStop({
      id: 'session-1',
      title: '实时记录',
      state: 'stopped',
      started_at: '2026-08-19T00:00:00Z',
      ended_at: '2026-08-19T00:05:00Z',
    })

    await waitFor(() => expect(screen.getByRole('button', { name: '开始新记录' })).toBeEnabled())
  })

  it('surfaces note persistence failures instead of silently keeping phantom notes', async () => {
    const onNotice = vi.fn()
    createNoteAdapter.mockRejectedValueOnce(new Error('db unavailable'))

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)

    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getAllByRole('button', { name: '新笔记' })[1])
    await user.type(screen.getByPlaceholderText('输入笔记内容...'), '这条不应该假装成功')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => expect(onNotice).toHaveBeenCalledWith('笔记保存失败，请稍后重试。'))
    expect(screen.getByPlaceholderText('输入笔记内容...')).toHaveValue('这条不应该假装成功')
    expect(screen.queryByText('这条不应该假装成功', { selector: '.note-card__content' })).not.toBeInTheDocument()
  })

  it('keeps a note visible when persisted deletion fails', async () => {
    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    await user.click(screen.getAllByRole('button', { name: '新笔记' })[1])
    await user.type(screen.getByPlaceholderText('输入笔记内容...'), '保留这条笔记')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(await screen.findByText('保留这条笔记')).toBeInTheDocument()

    deleteNoteAdapter.mockRejectedValueOnce(new Error('db unavailable'))
    await user.click(screen.getByRole('button', { name: '删除笔记' }))

    await waitFor(() => expect(onNotice).toHaveBeenCalledWith('笔记删除失败，请稍后重试。'))
    expect(screen.getByText('保留这条笔记')).toBeInTheDocument()
  })

  it('treats an empty LLM polish result as a visible failure', async () => {
    const onNotice = vi.fn()
    llmPolish.mockResolvedValue({
      original: '原始内容',
      polished: '',
    })

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-publication-committed', {
      revision_id: 'revision-polish-empty',
      session_id: 'session-1',
      revision_number: 1,
      provider: 'sense_voice',
      created_at: '2026-08-25T06:00:00Z',
      segments: [{
        segment_id: 'seg-1', chunk_id: 'chunk-1', capture_source_id: 'source-mic',
        start_ms: 1200, end_ms: 2200, source: 'microphone', text: '原始内容',
      }],
    })

    await user.click(await screen.findByRole('button', { name: '临时润色预览' }))

    await waitFor(() => expect(onNotice).toHaveBeenCalledWith('润色失败，请检查本地 LLM 是否可用。'))
    expect(screen.queryByRole('button', { name: '查看原始' })).not.toBeInTheDocument()
  })

  it('rejects fallback text instead of presenting it as a successful local LLM result', async () => {
    const onNotice = vi.fn()
    llmPolish.mockResolvedValue({
      original: '原始内容',
      polished: '规则替换后的内容',
      provider: 'mock',
      model: 'rule-based',
      fallback: 'rule_cleanup',
      error: 'ollama unavailable',
    })

    const { LiveCapture } = await import('./LiveCapture')
    const user = userEvent.setup()

    render(<LiveCapture onNotice={onNotice} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))

    emitEvent('asr-publication-committed', {
      revision_id: 'revision-polish-fallback',
      session_id: 'session-1',
      revision_number: 1,
      provider: 'sense_voice',
      created_at: '2026-08-25T06:00:00Z',
      segments: [{
        segment_id: 'seg-fallback', chunk_id: 'chunk-1', capture_source_id: 'source-mic',
        start_ms: 1200, end_ms: 2200, source: 'microphone', text: '原始内容',
      }],
    })

    await user.click(await screen.findByRole('button', { name: '临时润色预览' }))

    await waitFor(() => expect(onNotice).toHaveBeenCalledWith('润色失败，请检查本地 LLM 是否可用。'))
    expect(onNotice).not.toHaveBeenCalledWith('本地 LLM 润色完成。')
    expect(screen.queryByRole('button', { name: '查看原始' })).not.toBeInTheDocument()
  })

  it('cancels the browser demo timer when the user stops before demo data arrives', async () => {
    vi.useFakeTimers()
    isTauriRuntime.mockReturnValue(false)

    const onNotice = vi.fn()
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={onNotice} />)
    expect(screen.getByRole('button', { name: '播放示例转写' })).toBeInTheDocument()
    expect(screen.getAllByText('示例转写待播放').length).toBeGreaterThan(0)
    expect(screen.queryByText('浏览器演示数据，不会录音或保存。')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '播放示例转写' }))
    fireEvent.click(screen.getByRole('button', { name: '停止' }))
    await vi.advanceTimersByTimeAsync(1200)

    expect(screen.queryByText('我们今天先确认首版范围，重点是把基础闭环真正跑起来。')).not.toBeInTheDocument()
    vi.useRealTimers()
  })

  it('renames a suggested speaker inline without a system prompt', async () => {
    vi.useFakeTimers()
    isTauriRuntime.mockReturnValue(false)
    const { LiveCapture } = await import('./LiveCapture')

    render(<LiveCapture onNotice={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: '播放示例转写' }))
    await act(async () => {
      await Promise.resolve()
      await vi.advanceTimersByTimeAsync(1200)
    })
    fireEvent.click(screen.getByRole('button', { name: '可能是李娜？' }))

    const input = screen.getByRole('textbox', { name: '说话人名称' })
    expect(input).toHaveValue('李娜')
    fireEvent.change(input, { target: { value: '李娜确认' } })
    fireEvent.click(screen.getByRole('button', { name: '确认' }))
    expect(screen.getByText('李娜确认')).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: '说话人名称' })).not.toBeInTheDocument()
    vi.useRealTimers()
  })
})
