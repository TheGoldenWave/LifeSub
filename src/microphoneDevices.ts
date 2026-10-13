import { useEffect, useSyncExternalStore } from 'react'
import {
  getRecordingConfig,
  listMicrophoneDevices,
  probeMicrophone,
  setRecordingConfig,
  type CoreMicrophoneDevice,
  type CoreMicrophoneProbe,
  type CoreRecordingConfig,
} from './services/lifesub'
import type { CaptureMode } from './domain'

export type MicrophoneAvailability =
  | 'loading'
  | 'ready'
  | 'unselected'
  | 'disconnected'
  | 'empty'
  | 'error'

export interface MicrophoneDeviceState {
  phase: 'idle' | 'loading' | 'ready' | 'error'
  availability: MicrophoneAvailability
  devices: CoreMicrophoneDevice[]
  selectedUid: string | null
  selectedName: string | null
  captureMode: CaptureMode
  error: string | null
  refreshing: boolean
  saving: boolean
  locked: boolean
  probePhase: 'idle' | 'probing' | 'complete' | 'error'
  probeResult: CoreMicrophoneProbe | null
  probeError: string | null
  probeCompletedAt: number | null
}

export interface MicrophoneStartInput {
  mode: CaptureMode
  microphoneUid: string | null
  availability: MicrophoneAvailability
}

interface MicrophoneDeviceDependencies {
  loadConfig: () => Promise<CoreRecordingConfig>
  saveConfig: (config: CoreRecordingConfig) => Promise<void>
  listDevices: () => Promise<CoreMicrophoneDevice[]>
  probeDevice?: (uid: string) => Promise<CoreMicrophoneProbe>
}

const initialState: MicrophoneDeviceState = {
  phase: 'idle',
  availability: 'loading',
  devices: [],
  selectedUid: null,
  selectedName: null,
  captureMode: 'smart',
  error: null,
  refreshing: false,
  saving: false,
  locked: false,
  probePhase: 'idle',
  probeResult: null,
  probeError: null,
  probeCompletedAt: null,
}

export class MicrophoneDeviceController {
  private state = initialState
  private readonly listeners = new Set<() => void>()
  private loadPromise: Promise<void> | null = null
  private refreshGeneration = 0
  private selectionGeneration = 0
  private selectionQueue: Promise<void> = Promise.resolve()
  private probeGeneration = 0

  constructor(private readonly dependencies: MicrophoneDeviceDependencies) {}

  snapshot = () => this.state

  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  load = async () => {
    if (this.state.phase === 'ready') return
    if (this.loadPromise) return this.loadPromise

    const generation = ++this.refreshGeneration
    this.update({ phase: 'loading', error: null })
    this.loadPromise = Promise.all([
      this.dependencies.loadConfig(),
      this.dependencies.listDevices(),
    ]).then(([config, devices]) => {
      const patch: Partial<MicrophoneDeviceState> = {
        phase: 'ready',
        selectedUid: config.microphone_device_uid,
        selectedName: config.microphone_device_name,
        captureMode: normalizeCaptureMode(config.capture_mode),
        error: null,
      }
      if (generation === this.refreshGeneration) patch.devices = devices
      this.update(patch)
    }).catch((error: unknown) => {
      if (generation !== this.refreshGeneration) return
      this.update({
        phase: 'error',
        error: errorText(error, '无法读取麦克风设备。'),
      })
    }).finally(() => {
      this.loadPromise = null
    })
    return this.loadPromise
  }

  refresh = async () => {
    const generation = ++this.refreshGeneration
    this.update({ refreshing: true, error: null })
    try {
      const devices = await this.dependencies.listDevices()
      if (generation !== this.refreshGeneration) return
      const selectedConnected = devices.some((device) => device.uid === this.state.selectedUid && device.isConnected)
      if (!selectedConnected) ++this.probeGeneration
      this.update({
        phase: 'ready', devices, refreshing: false, error: null,
        ...(!selectedConnected ? { probePhase: 'idle' as const, probeResult: null, probeError: null, probeCompletedAt: null } : {}),
      })
    } catch (error) {
      if (generation !== this.refreshGeneration) return
      this.update({
        phase: 'error',
        refreshing: false,
        error: errorText(error, '刷新麦克风设备失败。'),
      })
    }
  }

  select = async (uid: string | null) => {
    if (this.state.locked) {
      this.update({ error: '录制期间不能切换麦克风。' })
      return
    }
    const device = uid === null
      ? null
      : this.state.devices.find((candidate) => candidate.uid === uid && candidate.isConnected)
    if (uid !== null && !device) {
      this.update({ error: '所选麦克风当前不可用。' })
      return
    }

    const generation = ++this.selectionGeneration
    ++this.probeGeneration
    const previous = {
      selectedUid: this.state.selectedUid,
      selectedName: this.state.selectedName,
    }
    const selectedName = device?.name ?? null
    this.update({
      selectedUid: uid,
      selectedName,
      saving: true,
      error: null,
      probePhase: 'idle',
      probeResult: null,
      probeError: null,
      probeCompletedAt: null,
    })

    const operation = this.selectionQueue.then(async () => {
      const config = await this.dependencies.loadConfig()
      await this.dependencies.saveConfig({
        ...config,
        microphone_device_uid: uid,
        microphone_device_name: selectedName,
      })
    })
    this.selectionQueue = operation.catch(() => undefined)

    try {
      await operation
      if (generation === this.selectionGeneration) {
        this.update({ saving: false, error: null })
      }
    } catch (error) {
      if (generation === this.selectionGeneration) {
        this.update({
          ...previous,
          saving: false,
          error: errorText(error, '麦克风选择保存失败。'),
        })
      }
    }
  }

  probe = async () => {
    const uid = this.state.selectedUid
    const dependency = this.dependencies.probeDevice
    if (this.state.locked || this.state.availability !== 'ready' || !uid || !dependency) {
      this.update({
        probePhase: 'error',
        probeResult: null,
        probeError: '所选麦克风当前无法测试。',
      })
      return
    }
    const generation = ++this.probeGeneration
    this.update({ probePhase: 'probing', probeResult: null, probeError: null, probeCompletedAt: null })
    try {
      const result = await dependency(uid)
      if (generation !== this.probeGeneration || this.state.selectedUid !== uid) return
      this.update({ probePhase: 'complete', probeResult: result, probeError: null, probeCompletedAt: Date.now() })
    } catch (error) {
      if (generation !== this.probeGeneration || this.state.selectedUid !== uid) return
      this.update({
        probePhase: 'error',
        probeResult: null,
        probeError: errorText(error, '麦克风测试失败。'),
        probeCompletedAt: null,
      })
    }
  }

  resolveStartInput = async (): Promise<MicrophoneStartInput> => {
    await this.selectionQueue
    const config = await this.dependencies.loadConfig()
    const selectionChanged = this.state.selectedUid !== config.microphone_device_uid
    if (selectionChanged) ++this.probeGeneration
    const synchronized = {
      ...this.state,
      selectedUid: config.microphone_device_uid,
      selectedName: config.microphone_device_name,
      captureMode: normalizeCaptureMode(config.capture_mode),
      ...(selectionChanged ? { probePhase: 'idle' as const, probeResult: null, probeError: null, probeCompletedAt: null } : {}),
    }
    this.update(synchronized)
    return {
      mode: synchronized.captureMode,
      microphoneUid: synchronized.selectedUid,
      availability: availabilityFor(synchronized),
    }
  }

  synchronizeConfig(config: CoreRecordingConfig) {
    const selectionChanged = this.state.selectedUid !== config.microphone_device_uid
    if (selectionChanged) ++this.probeGeneration
    this.update({
      selectedUid: config.microphone_device_uid,
      selectedName: config.microphone_device_name,
      captureMode: normalizeCaptureMode(config.capture_mode),
      ...(selectionChanged ? { probePhase: 'idle' as const, probeResult: null, probeError: null, probeCompletedAt: null } : {}),
    })
  }

  setLocked(locked: boolean) {
    this.update({ locked })
  }

  private update(patch: Partial<MicrophoneDeviceState>) {
    const candidate = { ...this.state, ...patch }
    this.state = {
      ...candidate,
      availability: availabilityFor(candidate),
    }
    this.listeners.forEach((listener) => listener())
  }
}

const controller = new MicrophoneDeviceController({
  loadConfig: getRecordingConfig,
  saveConfig: async (config) => {
    await setRecordingConfig(config)
  },
  listDevices: listMicrophoneDevices,
  probeDevice: probeMicrophone,
})

export function useMicrophoneDevices(enabled = true) {
  const state = useSyncExternalStore(controller.subscribe, controller.snapshot, controller.snapshot)
  useEffect(() => {
    if (enabled) void controller.load()
  }, [enabled])
  return {
    ...state,
    refresh: controller.refresh,
    select: controller.select,
    probe: controller.probe,
    resolveStartInput: controller.resolveStartInput,
    synchronizeConfig: (config: CoreRecordingConfig) => controller.synchronizeConfig(config),
    setLocked: (locked: boolean) => controller.setLocked(locked),
  }
}

function availabilityFor(state: Omit<MicrophoneDeviceState, 'availability'> | MicrophoneDeviceState): MicrophoneAvailability {
  if (state.phase === 'idle' || state.phase === 'loading') return 'loading'
  if (state.phase === 'error') return 'error'
  const connected = state.devices.filter((device) => device.isConnected)
  if (connected.length === 0) return state.selectedUid ? 'disconnected' : 'empty'
  if (!state.selectedUid) return 'unselected'
  return connected.some((device) => device.uid === state.selectedUid) ? 'ready' : 'disconnected'
}

function normalizeCaptureMode(mode: string): CaptureMode {
  if (mode === 'mic-only' || mode === 'mic_only') return 'mic-only'
  if (mode === 'system-only' || mode === 'system_only') return 'system-only'
  return 'smart'
}

function errorText(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === 'string' && error.trim()) return error
  return fallback
}
