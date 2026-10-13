import { useEffect, useMemo, useSyncExternalStore } from 'react'
import {
  cancelModelDownloadAdapter,
  installAsrModelAdapter,
  loadModelCatalog,
  retryModelDownloadAdapter,
} from './data/adapter'
import type { CoreAsrModel, CoreModelOperationAck } from './services/lifesub'

interface ModelManagerDependencies {
  list: () => Promise<CoreAsrModel[]>
  install: (modelId: string) => Promise<void>
  cancel: (downloadId: string) => Promise<CoreModelOperationAck>
  retry: (modelId: string, downloadId: string) => Promise<void>
  wait: () => Promise<void>
}

export interface ModelManagerState {
  models: CoreAsrModel[]
  loading: boolean
  refreshing: boolean
  error: string | null
  pendingModelId: string | null
  cancelRequestedDownloadId: string | null
}

const initialState: ModelManagerState = {
  models: [],
  loading: true,
  refreshing: false,
  error: null,
  pendingModelId: null,
  cancelRequestedDownloadId: null,
}

export class ModelManagerController {
  private state = initialState
  private readonly listeners = new Set<() => void>()
  private refreshGeneration = 0
  private operationGeneration = 0
  private disposed = false

  constructor(private readonly dependencies: ModelManagerDependencies) {}

  snapshot = () => this.state
  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  load = async () => {
    this.disposed = false
    this.update({ loading: true, error: null })
    const models = await this.fetchModels()
    if (!models) return
    this.update({ models, loading: false, refreshing: false })
    this.restorePersistedOperation(models)
  }

  refresh = async () => {
    this.update({ refreshing: true, error: null })
    const models = await this.fetchModels()
    if (!models) return
    this.update({ models, loading: false, refreshing: false })
    this.reconcilePending(models)
  }

  install = async (modelId: string) => {
    await this.runOperation(modelId, () => this.dependencies.install(modelId))
  }

  retry = async (modelId: string, downloadId: string) => {
    await this.runOperation(modelId, () => this.dependencies.retry(modelId, downloadId))
  }

  cancel = async (modelId: string, downloadId: string) => {
    try {
      const acknowledgement = await this.dependencies.cancel(downloadId)
      if (this.disposed || this.state.pendingModelId !== modelId) return
      if (acknowledgement.download_id === downloadId && acknowledgement.cancel_requested) {
        this.update({ cancelRequestedDownloadId: downloadId })
      }
    } catch (error) {
      if (!this.disposed) this.update({ error: errorText(error) })
    }
  }

  dispose() {
    this.disposed = true
    ++this.refreshGeneration
    ++this.operationGeneration
    this.listeners.clear()
  }

  private async runOperation(modelId: string, action: () => Promise<void>) {
    const generation = ++this.operationGeneration
    this.disposed = false
    this.update({
      pendingModelId: modelId,
      cancelRequestedDownloadId: null,
      error: null,
    })
    let settled = false
    let failure: unknown = null
    const operation = Promise.resolve()
      .then(action)
      .catch((error) => { failure = error })
      .finally(() => { settled = true })

    while (!settled && this.isOperationCurrent(generation)) {
      await this.dependencies.wait()
      if (!this.isOperationCurrent(generation)) break
      if (settled) break
      const models = await this.fetchModels()
      if (models && this.isOperationCurrent(generation)) this.update({ models })
    }
    await operation
    if (!this.isOperationCurrent(generation)) return
    const models = await this.fetchModels()
    if (models && this.isOperationCurrent(generation)) {
      this.update({ models })
      this.reconcilePending(models)
    }
    const cancelled = models
      ?.find((model) => model.model_id === modelId)
      ?.download?.state === 'cancelled'
    if (failure && !cancelled && this.isOperationCurrent(generation)) {
      this.update({ error: errorText(failure) })
    }
  }

  private restorePersistedOperation(models: CoreAsrModel[]) {
    const active = models.find((model) => isActiveDownload(model.download?.state))
    if (!active) return
    const generation = ++this.operationGeneration
    this.update({ pendingModelId: active.model_id, cancelRequestedDownloadId: null })
    void this.pollPersistedOperation(active.model_id, generation)
  }

  private async pollPersistedOperation(modelId: string, generation: number) {
    while (this.isOperationCurrent(generation)) {
      await this.dependencies.wait()
      if (!this.isOperationCurrent(generation)) return
      const models = await this.fetchModels()
      if (!this.isOperationCurrent(generation)) return
      if (!models) continue
      this.update({ models })
      const current = models.find((model) => model.model_id === modelId)
      if (!isActiveDownload(current?.download?.state)) {
        this.update({ pendingModelId: null, cancelRequestedDownloadId: null })
        return
      }
    }
  }

  private reconcilePending(models: CoreAsrModel[]) {
    const modelId = this.state.pendingModelId
    if (!modelId) {
      this.restorePersistedOperation(models)
      return
    }
    const current = models.find((model) => model.model_id === modelId)
    if (!isActiveDownload(current?.download?.state)) {
      this.update({ pendingModelId: null, cancelRequestedDownloadId: null })
    }
  }

  private async fetchModels(): Promise<CoreAsrModel[] | null> {
    const generation = ++this.refreshGeneration
    try {
      const models = await this.dependencies.list()
      if (this.disposed || generation !== this.refreshGeneration) return null
      return models
    } catch (error) {
      if (!this.disposed && generation === this.refreshGeneration) {
        this.update({ loading: false, refreshing: false, error: errorText(error) })
      }
      return null
    }
  }

  private isOperationCurrent(generation: number) {
    return !this.disposed && generation === this.operationGeneration
  }

  private update(patch: Partial<ModelManagerState>) {
    this.state = { ...this.state, ...patch }
    this.listeners.forEach((listener) => listener())
  }
}

export function useModelManagerController() {
  const controller = useMemo(() => new ModelManagerController({
    list: loadModelCatalog,
    install: installAsrModelAdapter,
    cancel: cancelModelDownloadAdapter,
    retry: retryModelDownloadAdapter,
    wait: () => new Promise((resolve) => setTimeout(resolve, 500)),
  }), [])
  const state = useSyncExternalStore(controller.subscribe, controller.snapshot, controller.snapshot)
  useEffect(() => {
    void controller.load()
    return () => controller.dispose()
  }, [controller])
  return { ...state, refresh: controller.refresh, install: controller.install, cancel: controller.cancel, retry: controller.retry }
}

function isActiveDownload(state: string | undefined) {
  return state != null && ['queued', 'downloading', 'verifying', 'installing', 'qualifying'].includes(state)
}

function errorText(error: unknown) {
  return error instanceof Error && error.message ? error.message : '模型操作失败'
}
