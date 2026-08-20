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

  it('imports audio and returns chunk with null job when auto-transcribe is off', async () => {
    const { importAudio } = await import('./lifesub')
    const session = { id: 'rec_1', title: '测试', state: 'stopped' as const, started_at: '2026-08-15T00:00:00Z', ended_at: null }
    invoke.mockResolvedValue({ chunk_id: 'chk_1', job_id: null })

    const result = await importAudio(session, '/path/to/audio.wav')

    expect(invoke).toHaveBeenCalledWith('import_audio_file', { session, path: '/path/to/audio.wav' })
    expect(result.chunkId).toBe('chk_1')
    expect(result.jobId).toBeNull()
  })

  it('imports audio and returns chunk with queued job when auto-transcribe is on', async () => {
    const { importAudio } = await import('./lifesub')
    const session = { id: 'rec_1', title: '测试', state: 'stopped' as const, started_at: '2026-08-15T00:00:00Z', ended_at: null }
    invoke.mockResolvedValue({ chunk_id: 'chk_1', job_id: 'job_1' })

    const result = await importAudio(session, '/path/to/audio.wav')

    expect(result.chunkId).toBe('chk_1')
    expect(result.jobId).toBe('job_1')
  })

  it('lists jobs for a session through the Rust command', async () => {
    const { listJobs } = await import('./lifesub')
    invoke.mockResolvedValue([{
      id: 'job_1', session_id: 'rec_1', chunk_id: 'chk_1',
      provider: 'sense_voice', model_id: 'sense-voice-small-int8-2024-07-17',
      state: 'queued', error_code: null, error_summary: null,
      created_at: '2026-08-15T00:00:00Z',
    }])

    const result = await listJobs('rec_1')

    expect(invoke).toHaveBeenCalledWith('list_asr_jobs', { sessionId: 'rec_1' })
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('job_1')
    expect(result[0].state).toBe('queued')
  })

  it('cancels an ASR job through the Rust command', async () => {
    const { cancelJob } = await import('./lifesub')
    invoke.mockResolvedValue(undefined)

    await cancelJob('job_1')

    expect(invoke).toHaveBeenCalledWith('cancel_asr_job', { jobId: 'job_1' })
  })

  it('retries an ASR job through the Rust command', async () => {
    const { retryJob } = await import('./lifesub')
    invoke.mockResolvedValue({ id: 'job_1', state: 'queued' })

    const result = await retryJob('job_1')

    expect(invoke).toHaveBeenCalledWith('retry_asr_job', { jobId: 'job_1' })
    expect(result.state).toBe('queued')
  })

  it('requests retranscription through the Rust command', async () => {
    const { retranscribe } = await import('./lifesub')
    invoke.mockResolvedValue({ id: 'job_2', session_id: 'rec_1', state: 'queued' })

    const result = await retranscribe('rec_1', 'chk_1')

    expect(invoke).toHaveBeenCalledWith('retranscribe_record', { sessionId: 'rec_1', chunkId: 'chk_1' })
    expect(result.id).toBe('job_2')
  })

  it('fetches a provider receipt for a job', async () => {
    const { getReceipt } = await import('./lifesub')
    invoke.mockResolvedValue({
      id: 'rcp_1', job_id: 'job_1', chunk_id: 'chk_1',
      provider: 'sense_voice', model_id: 'sense-voice-small-int8-2024-07-17',
      manifest_version: '2024-07-17',
      started_at: '2026-08-15T00:00:01Z', finished_at: '2026-08-15T00:00:05Z',
    })

    const result = await getReceipt('job_1')

    expect(invoke).toHaveBeenCalledWith('get_provider_receipt', { jobId: 'job_1' })
    expect(result.provider).toBe('sense_voice')
    expect(result.modelId).toBe('sense-voice-small-int8-2024-07-17')
  })

  it('fetches a session revision including provenance and receipt IDs', async () => {
    const { getSessionRevision } = await import('./lifesub')
    invoke.mockResolvedValue({
      id: 'rev_1', session_id: 'rec_1', number: 1,
      provider: 'sense_voice', created_at: '2026-08-15T00:00:00Z',
      provenance_status: 'verified_local_asr',
      receipt_ids: ['rcp_1'],
      segments: [{
        id: 'seg_1', start_ms: 0, end_ms: 5000,
        source: 'imported', text: '测试文本',
      }],
    })

    const result = await getSessionRevision('rec_1')

    expect(invoke).toHaveBeenCalledWith('get_session_revision', { sessionId: 'rec_1' })
    expect(result.provenance_status).toBe('verified_local_asr')
    expect(result.receipt_ids).toEqual(['rcp_1'])
    expect(result.segments).toHaveLength(1)
  })
})