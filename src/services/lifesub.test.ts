import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

describe('LifeSub desktop client', () => {
  beforeEach(() => {
    invoke.mockReset()
  })

  it('maps a capture transition to the Rust command', async () => {
    const { transitionCapture } = await import('./lifesub')
    const session = { id: 'rec_1', title: '测试', state: 'idle' as const, started_at: '2026-08-15T00:00:00Z', ended_at: null }
    invoke.mockResolvedValue({ ...session, state: 'recording' })

    const result = await transitionCapture(session, 'recording')

    expect(invoke).toHaveBeenCalledWith('transition_capture_session', { session, target: 'recording' })
    expect(result.state).toBe('recording')
  })

  it('resolves an Evidence URI through the Rust command', async () => {
    const { resolveEvidence } = await import('./lifesub')
    invoke.mockResolvedValue({ kind: 'audio', id: 'chk_1', start_seconds: 12, end_seconds: 18, revision: null })

    const result = await resolveEvidence('lifesub://audio/chk_1#t=12,18')

    expect(result.start_seconds).toBe(12)
    expect(invoke).toHaveBeenCalledWith('resolve_evidence', { uri: 'lifesub://audio/chk_1#t=12,18' })
  })
})
