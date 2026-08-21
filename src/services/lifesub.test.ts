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

  it('loads timeline records through the Rust command', async () => {
    const { listTimelineRecords } = await import('./lifesub')
    invoke.mockResolvedValue([
      {
        session: {
          id: 'rec_1',
          title: '首版讨论',
          state: 'stopped',
          started_at: '2026-08-19T08:00:00Z',
          ended_at: '2026-08-19T08:30:00Z',
        },
        chunks: [{
          id: 'chk_1',
          source: 'imported',
          audio_path: '/tmp/lifesub/audio.wav',
          integrity_state: 'available',
          error_code: null,
        }],
        latest_job: null,
        revisions: [],
        notes: [],
      },
    ])

    const result = await listTimelineRecords()

    expect(invoke).toHaveBeenCalledWith('list_timeline_records')
    expect(result[0].chunks[0].audio_path).toBe('/tmp/lifesub/audio.wav')
  })

  it('loads runtime info from the desktop service', async () => {
    const { getAppRuntimeInfo } = await import('./lifesub')
    invoke.mockResolvedValue({
      app_version: '0.2.7',
      tauri_version: '2.8.4',
      frontend_stack: 'React 19 + TypeScript',
      asr_runtime: 'sherpa-onnx 1.13.5',
    })

    const result = await getAppRuntimeInfo()

    expect(invoke).toHaveBeenCalledWith('get_app_runtime_info')
    expect(result.app_version).toBe('0.2.7')
  })

  it('loads the manifest-backed model catalog from the desktop service', async () => {
    const { listAsrModels } = await import('./lifesub')
    invoke.mockResolvedValue([
      {
        model_id: 'whisper-base',
        display_name: 'Whisper Base',
        provider: 'whisper',
        manifest_version: '1',
        bundle_identity: 'bundle-1',
        supported_languages: ['auto', 'en'],
        qualification_policy: 'structural_with_pinned_runtime',
        runtime_family: 'sherpa_onnx',
        runtime_version: '1.13.5',
        artifact_count: 3,
        total_bytes: 293277543,
        license_spdx: 'MIT',
        installation_state: 'runtime_qualified',
        selectable: true,
        installable: true,
        executable: true,
        reason_code: null,
        last_error_code: null,
        download: null,
      },
    ])

    const result = await listAsrModels()

    expect(invoke).toHaveBeenCalledWith('list_asr_models')
    expect(result[0].display_name).toBe('Whisper Base')
  })

  it('uses the single desktop import command', async () => {
    const { importAudioRecord } = await import('./lifesub')
    invoke.mockResolvedValue({
      session: { id: 'rec_1', title: 'sample', state: 'stopped', started_at: '2026-08-19T08:00:00Z', ended_at: '2026-08-19T08:01:00Z' },
      chunk: { id: 'chk_1', session_id: 'rec_1', source: 'imported', path: 'audio/sample.wav', sha256: 'a'.repeat(64), byte_length: 128 },
      job: { id: 'asr_1', state: 'queued', error_code: null, error_summary: null, chunk_id: 'chk_1' },
    })

    const result = await importAudioRecord('/tmp/sample.wav', 'sample')

    expect(invoke).toHaveBeenCalledWith('import_audio_record', { path: '/tmp/sample.wav', title: 'sample' })
    expect(result.job?.state).toBe('queued')
  })

  it('creates a manual revision through the dedicated desktop command', async () => {
    const { createManualRevision } = await import('./lifesub')
    invoke.mockResolvedValue({ id: 'tr_2', session_id: 'rec_1', number: 2, provider: 'manual', created_at: '2026-08-19T08:02:00Z', segments: [] })

    await createManualRevision('rec_1', [
      { id: 'seg_1', start_ms: 0, end_ms: 1000, source: 'imported', text: '编辑后文本' },
    ])

    expect(invoke).toHaveBeenCalledWith('create_manual_revision', {
      sessionId: 'rec_1',
      segments: [{ id: 'seg_1', start_ms: 0, end_ms: 1000, source: 'imported', text: '编辑后文本' }],
    })
  })
})
