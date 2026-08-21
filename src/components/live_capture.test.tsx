import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
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
const isTauriRuntime = vi.fn()

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
  isTauriRuntime,
}))

vi.mock('../data/adapter', () => ({
  loadNotes,
  createNoteAdapter,
  deleteNoteAdapter,
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
    isTauriRuntime.mockReset()
    loadNotes.mockReset()
    createNoteAdapter.mockReset()
    deleteNoteAdapter.mockReset()

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
    expect(screen.queryByText('我们今天先确认首版范围，重点是把基础闭环真正跑起来。')).not.toBeInTheDocument()
    expect(onNotice).toHaveBeenCalledWith('桌面实时采集未接通，已阻止演示数据回退。')
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

    emitEvent('asr-live-segment', {
      id: 'seg-1',
      startMs: 1200,
      speaker: { id: 'spk-1', label: '我', source: 'manual', voiceprintId: null },
      text: '原始内容',
      completed: true,
    })

    await user.click(await screen.findByRole('button', { name: 'AI 润色' }))

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

    emitEvent('asr-live-segment', {
      id: 'seg-fallback',
      startMs: 1200,
      speaker: { id: 'spk-1', label: '我', source: 'manual', voiceprintId: null },
      text: '原始内容',
      completed: true,
    })

    await user.click(await screen.findByRole('button', { name: 'AI 润色' }))

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
    fireEvent.click(screen.getByRole('button', { name: '开始演示' }))
    fireEvent.click(screen.getByRole('button', { name: '停止' }))
    await vi.advanceTimersByTimeAsync(1200)

    expect(screen.queryByText('我们今天先确认首版范围，重点是把基础闭环真正跑起来。')).not.toBeInTheDocument()
    vi.useRealTimers()
  })
})
