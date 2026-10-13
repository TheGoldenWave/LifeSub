import { describe, expect, it, vi } from 'vitest'
import type { CoreMicrophoneDevice, CoreMicrophoneProbe, CoreRecordingConfig } from './services/lifesub'
import { MicrophoneDeviceController } from './microphoneDevices'

const baseConfig: CoreRecordingConfig = {
  capture_mode: 'smart',
  im_detection_enabled: true,
  im_apps: [],
  detection_delay_secs: 3,
  recovery_delay_secs: 5,
  sample_rate: 16_000,
  storage_path: '~/.lifesub/recordings/',
  microphone_device_uid: null,
  microphone_device_name: null,
}

const builtIn: CoreMicrophoneDevice = {
  uid: 'built-in-1',
  name: 'MacBook Pro 麦克风',
  isDefault: true,
  isConnected: true,
  transport: 'built_in',
}

const usb: CoreMicrophoneDevice = {
  uid: 'usb-1',
  name: 'Studio USB Microphone',
  isDefault: false,
  isConnected: true,
  transport: 'usb',
}

describe('MicrophoneDeviceController', () => {
  it('keeps the saved name visible when the selected device is disconnected', async () => {
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({
        ...baseConfig,
        microphone_device_uid: usb.uid,
        microphone_device_name: usb.name,
      }),
      saveConfig: vi.fn(),
      listDevices: vi.fn().mockResolvedValue([builtIn]),
    })

    await controller.load()

    expect(controller.snapshot().availability).toBe('disconnected')
    expect(controller.snapshot().selectedName).toBe(usb.name)
    expect(controller.snapshot().selectedUid).toBe(usb.uid)
  })

  it('ignores an older refresh that resolves after the newest request', async () => {
    let resolveOlder: ((devices: CoreMicrophoneDevice[]) => void) | undefined
    const listDevices = vi.fn()
      .mockResolvedValueOnce([builtIn])
      .mockImplementationOnce(() => new Promise<CoreMicrophoneDevice[]>((resolve) => {
        resolveOlder = resolve
      }))
      .mockResolvedValueOnce([usb])
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(baseConfig),
      saveConfig: vi.fn(),
      listDevices,
    })
    await controller.load()

    const older = controller.refresh()
    const newest = controller.refresh()
    await newest
    resolveOlder?.([builtIn])
    await older

    expect(controller.snapshot().devices).toEqual([usb])
  })

  it('does not let the initial load overwrite a newer refresh result', async () => {
    let resolveInitial: ((devices: CoreMicrophoneDevice[]) => void) | undefined
    const listDevices = vi.fn()
      .mockImplementationOnce(() => new Promise<CoreMicrophoneDevice[]>((resolve) => {
        resolveInitial = resolve
      }))
      .mockResolvedValueOnce([usb])
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(baseConfig),
      saveConfig: vi.fn(),
      listDevices,
    })

    const initial = controller.load()
    await controller.refresh()
    resolveInitial?.([builtIn])
    await initial

    expect(controller.snapshot().devices).toEqual([usb])
  })

  it('does not let a stale initial-load failure replace a newer ready refresh', async () => {
    let rejectInitial: ((error: Error) => void) | undefined
    const listDevices = vi.fn()
      .mockImplementationOnce(() => new Promise<CoreMicrophoneDevice[]>((_resolve, reject) => {
        rejectInitial = reject
      }))
      .mockResolvedValueOnce([usb])
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(baseConfig),
      saveConfig: vi.fn(),
      listDevices,
    })

    const initial = controller.load()
    await controller.refresh()
    rejectInitial?.(new Error('stale discovery failed'))
    await initial

    expect(controller.snapshot().phase).toBe('ready')
    expect(controller.snapshot().error).toBeNull()
    expect(controller.snapshot().devices).toEqual([usb])
  })

  it('serializes rapid selections so the last choice is persisted last', async () => {
    const writes: Array<string | null> = []
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(baseConfig),
      listDevices: vi.fn().mockResolvedValue([builtIn, usb]),
      saveConfig: vi.fn().mockImplementation(async (config: CoreRecordingConfig) => {
        writes.push(config.microphone_device_uid)
      }),
    })
    await controller.load()

    const first = controller.select(builtIn.uid)
    const second = controller.select(usb.uid)
    await Promise.all([first, second])

    expect(writes).toEqual([builtIn.uid, usb.uid])
    expect(controller.snapshot().selectedUid).toBe(usb.uid)
    expect(controller.snapshot().availability).toBe('ready')
  })

  it('rejects selection writes while capture lifecycle is locked', async () => {
    const saveConfig = vi.fn()
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(baseConfig),
      listDevices: vi.fn().mockResolvedValue([builtIn]),
      saveConfig,
    })
    await controller.load()
    controller.setLocked(true)

    await controller.select(builtIn.uid)

    expect(saveConfig).not.toHaveBeenCalled()
    expect(controller.snapshot().selectedUid).toBeNull()
    expect(controller.snapshot().locked).toBe(true)
  })

  it('resolves the last persisted microphone after an optimistic selection save fails', async () => {
    const persisted = {
      ...baseConfig,
      capture_mode: 'mic_only',
      microphone_device_uid: builtIn.uid,
      microphone_device_name: builtIn.name,
    }
    let rejectSave: ((error: Error) => void) | undefined
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue(persisted),
      listDevices: vi.fn().mockResolvedValue([builtIn, usb]),
      saveConfig: vi.fn().mockImplementation(() => new Promise<void>((_resolve, reject) => {
        rejectSave = reject
      })),
    })
    await controller.load()

    const selection = controller.select(usb.uid)
    const resolution = controller.resolveStartInput()
    await vi.waitFor(() => expect(rejectSave).toBeTypeOf('function'))
    rejectSave?.(new Error('selection save failed'))

    await selection
    await expect(resolution).resolves.toEqual({
      mode: 'mic-only',
      microphoneUid: builtIn.uid,
      availability: 'ready',
    })
    expect(controller.snapshot().selectedUid).toBe(builtIn.uid)
  })

  it('blocks microphone start resolution while device discovery is not ready', async () => {
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({
        ...baseConfig,
        capture_mode: 'mic_only',
        microphone_device_uid: builtIn.uid,
        microphone_device_name: builtIn.name,
      }),
      listDevices: vi.fn(),
      saveConfig: vi.fn(),
    })

    await expect(controller.resolveStartInput()).resolves.toEqual({
      mode: 'mic-only',
      microphoneUid: builtIn.uid,
      availability: 'loading',
    })
  })

  it('resolves system-only mode without waiting for microphone discovery', async () => {
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({
        ...baseConfig,
        capture_mode: 'system_only',
      }),
      listDevices: vi.fn(),
      saveConfig: vi.fn(),
    })

    await expect(controller.resolveStartInput()).resolves.toEqual({
      mode: 'system-only',
      microphoneUid: null,
      availability: 'loading',
    })
  })

  it('probes the exact selected microphone and records compatible readiness', async () => {
    const compatible: CoreMicrophoneProbe = {
      uid: builtIn.uid,
      name: builtIn.name,
      transport: 'built_in',
      input_sample_rate_hz: 44_100,
      input_channel_count: 1,
      read_back_uid: builtIn.uid,
      first_frame_sample_count: 128,
      elapsed_ms: 12,
      result: 'compatible',
    }
    const probeDevice = vi.fn().mockResolvedValue(compatible)
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({
        ...baseConfig,
        microphone_device_uid: builtIn.uid,
        microphone_device_name: builtIn.name,
      }),
      listDevices: vi.fn().mockResolvedValue([builtIn]),
      saveConfig: vi.fn(),
      probeDevice,
    })
    await controller.load()

    await controller.probe()

    expect(probeDevice).toHaveBeenCalledWith(builtIn.uid)
    expect(controller.snapshot().probePhase).toBe('complete')
    expect(controller.snapshot().probeResult).toEqual(compatible)
  })

  it('discards a stale probe result after the selected microphone changes', async () => {
    let resolveProbe: ((result: CoreMicrophoneProbe) => void) | undefined
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({
        ...baseConfig,
        microphone_device_uid: builtIn.uid,
        microphone_device_name: builtIn.name,
      }),
      listDevices: vi.fn().mockResolvedValue([builtIn, usb]),
      saveConfig: vi.fn(),
      probeDevice: vi.fn().mockImplementation(() => new Promise<CoreMicrophoneProbe>((resolve) => {
        resolveProbe = resolve
      })),
    })
    await controller.load()

    const pending = controller.probe()
    await controller.select(usb.uid)
    resolveProbe?.({
      uid: builtIn.uid,
      name: builtIn.name,
      transport: 'built_in',
      input_sample_rate_hz: 44_100,
      input_channel_count: 1,
      read_back_uid: builtIn.uid,
      first_frame_sample_count: 128,
      elapsed_ms: 12,
      result: 'compatible',
    })
    await pending

    expect(controller.snapshot().selectedUid).toBe(usb.uid)
    expect(controller.snapshot().probeResult).toBeNull()
    expect(controller.snapshot().probePhase).toBe('idle')
  })

  it('timestamps a completed probe and clears that readiness after the device disconnects', async () => {
    const listDevices = vi.fn().mockResolvedValueOnce([builtIn]).mockResolvedValueOnce([])
    const controller = new MicrophoneDeviceController({
      loadConfig: vi.fn().mockResolvedValue({ ...baseConfig, microphone_device_uid: builtIn.uid, microphone_device_name: builtIn.name }),
      listDevices,
      saveConfig: vi.fn(),
      probeDevice: vi.fn().mockResolvedValue({
        uid: builtIn.uid, name: builtIn.name, transport: 'built_in', input_sample_rate_hz: 44_100,
        input_channel_count: 1, read_back_uid: builtIn.uid, first_frame_sample_count: 128,
        elapsed_ms: 12, result: 'compatible',
      }),
    })
    await controller.load()
    await controller.probe()
    expect(controller.snapshot().probeCompletedAt).toBeGreaterThan(0)

    await controller.refresh()
    expect(controller.snapshot().availability).toBe('disconnected')
    expect(controller.snapshot().probeResult).toBeNull()
    expect(controller.snapshot().probeCompletedAt).toBeNull()
  })

  it('invalidates a probe if persisted selection changes before start resolution', async () => {
    const loadConfig = vi.fn()
      .mockResolvedValueOnce({ ...baseConfig, microphone_device_uid: builtIn.uid, microphone_device_name: builtIn.name })
      .mockResolvedValueOnce({ ...baseConfig, microphone_device_uid: usb.uid, microphone_device_name: usb.name })
    const controller = new MicrophoneDeviceController({
      loadConfig,
      listDevices: vi.fn().mockResolvedValue([builtIn, usb]),
      saveConfig: vi.fn(),
      probeDevice: vi.fn().mockResolvedValue({
        uid: builtIn.uid, name: builtIn.name, transport: 'built_in', input_sample_rate_hz: 44_100,
        input_channel_count: 1, read_back_uid: builtIn.uid, first_frame_sample_count: 128,
        elapsed_ms: 12, result: 'compatible',
      }),
    })
    await controller.load()
    await controller.probe()

    await controller.resolveStartInput()
    expect(controller.snapshot().selectedUid).toBe(usb.uid)
    expect(controller.snapshot().probeResult).toBeNull()
    expect(controller.snapshot().probeCompletedAt).toBeNull()
  })
})
