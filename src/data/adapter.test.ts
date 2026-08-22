import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

describe('timeline adapter', () => {
  beforeEach(() => {
    invoke.mockReset()
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('returns demo records outside the Tauri runtime', async () => {
    const { loadTimelineRecords } = await import('./adapter')

    const result = await loadTimelineRecords()

    expect(result[0]?.title).toContain('LifeSub')
  })

  it('maps desktop timeline records from the Catalog command', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockResolvedValue([
      {
        session: {
          id: 'rec_1',
          title: '真实导入样本',
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
        latest_job: {
          id: 'asr_1',
          chunk_id: 'chk_1',
          state: 'succeeded',
          error_code: null,
          error_summary: null,
        },
        revisions: [
          {
            id: 'tr_1',
            session_id: 'rec_1',
            number: 1,
            provider: 'sense_voice',
            created_at: '2026-08-19T08:31:00Z',
            segments: [
              {
                id: 'seg_1',
                start_ms: 0,
                end_ms: 4200,
                source: 'imported',
                text: '真实转写',
                chunk_id: 'chk_1',
                chunk_start_ms: 0,
                chunk_end_ms: 4200,
              },
            ],
          },
        ],
        notes: [],
      },
    ])

    const { loadTimelineRecords } = await import('./adapter')
    const result = await loadTimelineRecords()

    expect(result).toHaveLength(1)
    expect(result[0]?.chunks[0]?.audioPath).toBe('/tmp/lifesub/audio.wav')
    expect(result[0]?.revision.provider).toBe('sense_voice')
    expect(result[0]?.revisions).toHaveLength(1)
    expect(result[0]?.latestJob?.state).toBe('succeeded')
  })

  it('does not fall back to demo timeline data on desktop load failure', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockRejectedValueOnce(new Error('catalog unavailable'))
    const { loadTimelineRecords } = await import('./adapter')

    await expect(loadTimelineRecords()).rejects.toThrow('catalog unavailable')
  })

  it('does not fall back to demo stats on desktop load failure', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockRejectedValueOnce(new Error('stats unavailable'))
    const { loadStats } = await import('./adapter')

    await expect(loadStats()).rejects.toThrow('stats unavailable')
  })

  it('rethrows Tauri ASR settings load failures instead of falling back to defaults', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockRejectedValueOnce(new Error('desktop load failed'))
    const { loadAsrConfig } = await import('./adapter')

    await expect(loadAsrConfig()).rejects.toThrow('desktop load failed')
  })

  it('rethrows Tauri recording settings load failures instead of falling back to defaults', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockRejectedValueOnce(new Error('desktop recording failed'))
    const { loadRecordingConfig } = await import('./adapter')

    await expect(loadRecordingConfig()).rejects.toThrow('desktop recording failed')
  })

  it('rethrows Tauri voiceprint load failures instead of falling back to demo data', async () => {
    ;(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    invoke.mockRejectedValueOnce(new Error('desktop voiceprint failed'))
    const { loadVoiceprints } = await import('./adapter')

    await expect(loadVoiceprints()).rejects.toThrow('desktop voiceprint failed')
  })
})
