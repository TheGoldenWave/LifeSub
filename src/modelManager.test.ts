import { describe, expect, it, vi } from 'vitest'
import type { CoreAsrModel, CoreModelDownload } from './services/lifesub'
import { ModelManagerController } from './modelManager'

const activeDownload: CoreModelDownload = {
  id: 'mdl_1', state: 'downloading', downloaded_bytes: 50, expected_bytes: 100,
  artifact_index: 1, artifact_count: 2, error_code: null, error_summary: null,
}

function model(download: CoreModelDownload | null = null): CoreAsrModel {
  return {
    model_id: 'sense-voice-small', display_name: 'SenseVoice Small', provider: 'sense_voice',
    manifest_version: '1', bundle_identity: 'a'.repeat(64), supported_languages: ['auto', 'zh'],
    qualification_policy: 'structural_with_pinned_runtime', runtime_family: 'sherpa_onnx',
    runtime_version: '1.13.5', artifact_count: 2, total_bytes: 100, license_spdx: 'MIT',
    installation_state: 'not_installed', selectable: false, installable: true, executable: false,
    reason_code: 'model_not_installed', last_error_code: null, download,
  }
}

describe('ModelManagerController', () => {
  it('discards an older catalog poll that resolves after the newest refresh', async () => {
    let resolveOld: ((models: CoreAsrModel[]) => void) | undefined
    const list = vi.fn()
      .mockResolvedValueOnce([model()])
      .mockImplementationOnce(() => new Promise<CoreAsrModel[]>((resolve) => { resolveOld = resolve }))
      .mockResolvedValueOnce([{ ...model(), display_name: 'Newest' }])
    const controller = controllerWith({ list })
    await controller.load()

    const old = controller.refresh()
    const newest = controller.refresh()
    await newest
    resolveOld?.([{ ...model(), display_name: 'Stale' }])
    await old

    expect(controller.snapshot().models[0].display_name).toBe('Newest')
  })

  it('restores polling from a persisted active download after remount', async () => {
    let releaseWait: (() => void) | undefined
    const list = vi.fn()
      .mockResolvedValueOnce([model(activeDownload)])
      .mockResolvedValueOnce([model({ ...activeDownload, state: 'cancelled' })])
    const controller = controllerWith({
      list,
      wait: () => new Promise<void>((resolve) => { releaseWait = resolve }),
    })

    await controller.load()
    expect(controller.snapshot().pendingModelId).toBe('sense-voice-small')
    releaseWait?.()
    await vi.waitFor(() => expect(controller.snapshot().pendingModelId).toBeNull())
    expect(controller.snapshot().models[0].download?.state).toBe('cancelled')
  })

  it('keeps cancellation pending until Catalog acknowledges cancelled', async () => {
    const cancel = vi.fn().mockResolvedValue({ download_id: 'mdl_1', cancel_requested: true })
    const controller = controllerWith({ list: vi.fn().mockResolvedValue([model(activeDownload)]), cancel })
    await controller.load()

    await controller.cancel('sense-voice-small', 'mdl_1')

    expect(cancel).toHaveBeenCalledWith('mdl_1')
    expect(controller.snapshot().pendingModelId).toBe('sense-voice-small')
    expect(controller.snapshot().cancelRequestedDownloadId).toBe('mdl_1')
    controller.dispose()
  })

  it('retries the same durable download id and ignores completion after dispose', async () => {
    let resolveRetry: (() => void) | undefined
    let releasePoll: (() => void) | undefined
    const retry = vi.fn().mockImplementation(() => new Promise<void>((resolve) => { resolveRetry = resolve }))
    const list = vi.fn().mockResolvedValue([model({ ...activeDownload, state: 'failed' })])
    const controller = controllerWith({
      list,
      retry,
      wait: () => new Promise<void>((resolve) => { releasePoll = resolve }),
    })
    await controller.load()

    const operation = controller.retry('sense-voice-small', 'mdl_1')
    await vi.waitFor(() => expect(resolveRetry).toBeTypeOf('function'))
    controller.dispose()
    resolveRetry?.()
    releasePoll?.()
    await operation

    expect(retry).toHaveBeenCalledWith('sense-voice-small', 'mdl_1')
    expect(controller.snapshot().pendingModelId).toBe('sense-voice-small')
  })

  it('converges synchronous adapter failures into controller error state', async () => {
    const controller = controllerWith({
      list: vi.fn().mockResolvedValue([model()]),
      install: (() => { throw new Error('sync install failure') }) as any,
      wait: () => Promise.resolve(),
    })
    await controller.load()

    await controller.install('sense-voice-small')

    expect(controller.snapshot().error).toBe('sync install failure')
    expect(controller.snapshot().pendingModelId).toBeNull()
  })
})

function controllerWith(overrides: Partial<ConstructorParameters<typeof ModelManagerController>[0]>) {
  return new ModelManagerController({
    list: vi.fn().mockResolvedValue([]),
    install: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn().mockResolvedValue({ download_id: 'mdl_1', cancel_requested: true }),
    retry: vi.fn().mockResolvedValue(undefined),
    wait: () => new Promise((resolve) => setTimeout(resolve, 1)),
    ...overrides,
  })
}
