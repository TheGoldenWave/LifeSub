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

  it('loads settings capabilities through the dedicated desktop command', async () => {
    const { getSettingsCapabilities } = await import('./lifesub') as any
    invoke.mockResolvedValue({
      production_capture_output_sample_rate_hz: 16000,
      data_directory_path: '/Users/test/Library/Application Support/LifeSub',
      im_detection_available: false,
      supported_capture_modes: ['smart', 'mic_only', 'system_only'],
    })

    const result = await getSettingsCapabilities()

    expect(invoke).toHaveBeenCalledWith('get_settings_capabilities')
    expect(result.supported_capture_modes).toEqual(['smart', 'mic_only', 'system_only'])
  })

  it('installs an exact manifest-backed ASR model through the desktop service', async () => {
    const { installAsrModel } = await import('./lifesub')
    invoke.mockResolvedValue({
      model_id: 'sense-voice-small-int8-2024-07-17',
      installation_state: 'runtime_qualified',
    })

    const result = await installAsrModel('sense-voice-small-int8-2024-07-17')

    expect(invoke).toHaveBeenCalledWith('install_asr_model', {
      modelId: 'sense-voice-small-int8-2024-07-17',
    })
    expect(result.installation_state).toBe('runtime_qualified')
  })

  it('requests cancellation and retries the exact durable model download', async () => {
    const { cancelModelDownload, retryModelDownload } = await import('./lifesub')
    invoke
      .mockResolvedValueOnce({ download_id: 'mdl_1', cancel_requested: true })
      .mockResolvedValueOnce({ model_id: 'sense-voice-small', installation_state: 'runtime_qualified' })

    const ack = await cancelModelDownload('mdl_1')
    await retryModelDownload('sense-voice-small', 'mdl_1')

    expect(ack.cancel_requested).toBe(true)
    expect(invoke).toHaveBeenNthCalledWith(1, 'cancel_model_download', { downloadId: 'mdl_1' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'retry_model_download', { modelId: 'sense-voice-small', downloadId: 'mdl_1' })
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

  it('passes the selected microphone UID into the desktop capture start command', async () => {
    const { startStreamingCapture } = await import('./lifesub')
    invoke.mockResolvedValue({ state: 'started' })

    await startStreamingCapture('rec_1', 'mic-only', 'mic-uid-1')

    expect(invoke).toHaveBeenCalledWith('start_streaming_capture', {
      sessionId: 'rec_1',
      captureMode: 'mic-only',
      microphoneDeviceUid: 'mic-uid-1',
    })
  })

  it('lists microphone devices through the desktop command', async () => {
    const { listMicrophoneDevices } = await import('./lifesub')
    invoke.mockResolvedValue([
      {
        uid: 'mic-uid-1',
        name: 'Built-in Microphone',
        isDefault: true,
        isConnected: true,
        transport: 'built_in',
      },
    ])

    const result = await listMicrophoneDevices()

    expect(invoke).toHaveBeenCalledWith('list_microphone_devices')
    expect(result[0].uid).toBe('mic-uid-1')
  })

  it('probes the exact selected microphone through the desktop command', async () => {
    const { probeMicrophone } = await import('./lifesub')
    invoke.mockResolvedValue({
      uid: 'mic-uid-1',
      name: 'Built-in Microphone',
      transport: 'built_in',
      input_sample_rate_hz: 44100,
      input_channel_count: 1,
      read_back_uid: 'mic-uid-1',
      first_frame_sample_count: 128,
      elapsed_ms: 12,
      result: 'compatible',
    })

    const result = await probeMicrophone('mic-uid-1')

    expect(invoke).toHaveBeenCalledWith('probe_microphone', { microphoneDeviceUid: 'mic-uid-1' })
    expect(result.result).toBe('compatible')
  })

  it('loads, exports and opens allowlisted diagnostics actions', async () => {
    const { getDiagnostics, writeDiagnosticsExport, writeRecordMarkdownExport, openMacosPrivacySettings } = await import('./lifesub')
    const diagnostics = { schema: 'lifesub.diagnostics.v1' }
    invoke.mockResolvedValueOnce(diagnostics).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined)

    expect(await getDiagnostics()).toEqual(diagnostics)
    await writeDiagnosticsExport('/tmp/diagnostics.json', diagnostics as any)
    await writeRecordMarkdownExport('/tmp/record.md', 'rec_1', 2)
    await openMacosPrivacySettings('microphone')

    expect(invoke).toHaveBeenNthCalledWith(1, 'get_diagnostics')
    expect(invoke).toHaveBeenNthCalledWith(2, 'write_diagnostics_export', { path: '/tmp/diagnostics.json', snapshot: diagnostics })
    expect(invoke).toHaveBeenNthCalledWith(3, 'write_record_markdown_export', { path: '/tmp/record.md', sessionId: 'rec_1', revisionNumber: 2 })
    expect(invoke).toHaveBeenNthCalledWith(4, 'open_macos_privacy_settings', { target: 'microphone' })
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

  it('persists whisper task in the canonical ASR config payload', async () => {
    const { setAsrConfig } = await import('./lifesub') as any
    invoke.mockResolvedValue(undefined)

    await setAsrConfig({
      provider: 'whisper',
      model_id: 'whisper-base',
      language: 'auto',
      auto_transcribe: true,
      threads: 4,
      vad_enabled: true,
      vad_min_speech_ms: 300,
      vad_silence_ms: 800,
      itn_enabled: false,
      whisper_task: 'translate',
    })

    expect(invoke).toHaveBeenCalledWith('set_asr_config', {
      config: {
        provider: 'whisper',
        model_id: 'whisper-base',
        language: 'auto',
        auto_transcribe: true,
        threads: 4,
        vad_enabled: true,
        vad_min_speech_ms: 300,
        vad_silence_ms: 800,
        itn_enabled: false,
        whisper_task: 'translate',
      },
    })
  })
})

describe('Browser preview fallback', () => {
  beforeEach(() => {
    invoke.mockReset()
    Reflect.deleteProperty(window as unknown as Record<string, unknown>, '__TAURI_INTERNALS__')
  })

  it('returns truthful non-desktop settings capabilities without invoking Tauri', async () => {
    vi.resetModules()
    const adapter = await import('../data/adapter') as any

    const result = await adapter.loadSettingsCapabilities()

    expect(invoke).not.toHaveBeenCalled()
    expect(result).toEqual({
      production_capture_output_sample_rate_hz: 16000,
      data_directory_path: '浏览器预览不提供真实 LifeSub 数据目录',
      im_detection_available: false,
      supported_capture_modes: ['smart', 'mic_only', 'system_only'],
    })
  })
})
