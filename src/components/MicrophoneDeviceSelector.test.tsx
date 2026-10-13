import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { MicrophoneDeviceSelector } from './MicrophoneDeviceSelector'
import type { MicrophoneDeviceState } from '../microphoneDevices'

const readyState: MicrophoneDeviceState = {
  phase: 'ready',
  availability: 'ready',
  devices: [
    {
      uid: 'built-in-1',
      name: 'MacBook Pro 麦克风',
      isDefault: true,
      isConnected: true,
      transport: 'built_in',
    },
  ],
  selectedUid: 'built-in-1',
  selectedName: 'MacBook Pro 麦克风',
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

describe('MicrophoneDeviceSelector', () => {
  it.each([
    ['loading', { phase: 'loading', availability: 'loading' }, '正在检测麦克风'],
    ['unselected', { selectedUid: null, selectedName: null, availability: 'unselected' }, '录制前请选择一个麦克风'],
    ['empty', { devices: [], selectedUid: null, selectedName: null, availability: 'empty' }, '未检测到可用麦克风'],
    ['error', { phase: 'error', availability: 'error', error: '设备服务不可用' }, '设备服务不可用'],
  ])('renders the %s state', (_label, patch, expected) => {
    renderSelector({ ...readyState, ...patch } as MicrophoneDeviceState)
    expect(screen.getByRole('status')).toHaveTextContent(expected)
  })

  it('keeps a disconnected saved microphone visible without falling back', () => {
    renderSelector({
      ...readyState,
      devices: [],
      selectedUid: 'usb-1',
      selectedName: 'Studio USB Microphone With A Very Long Device Name',
      availability: 'disconnected',
    })

    expect(screen.getByRole('combobox', { name: '收音麦克风' })).toHaveValue('usb-1')
    expect(screen.getByText('Studio USB Microphone With A Very Long Device Name（已断开）')).toBeInTheDocument()
  })

  it('locks selection while recording and still exposes a named refresh action', async () => {
    const refresh = vi.fn()
    renderSelector(readyState, { locked: true, onRefresh: refresh })

    expect(screen.getByRole('combobox', { name: '收音麦克风' })).toBeDisabled()
    expect(screen.getByText('录制中不可切换')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '刷新麦克风列表' })).toBeDisabled()
  })

  it('reports an explicit selection and refresh', async () => {
    const user = userEvent.setup()
    const select = vi.fn()
    const refresh = vi.fn()
    renderSelector({ ...readyState, selectedUid: null, selectedName: null, availability: 'unselected' }, {
      onSelect: select,
      onRefresh: refresh,
    })

    await user.selectOptions(screen.getByRole('combobox', { name: '收音麦克风' }), 'built-in-1')
    await user.click(screen.getByRole('button', { name: '刷新麦克风列表' }))

    expect(select).toHaveBeenCalledWith('built-in-1')
    expect(refresh).toHaveBeenCalledOnce()
  })

  it('tests the selected microphone and reports first-frame compatibility', async () => {
    const user = userEvent.setup()
    const probe = vi.fn()
    renderSelector({
      ...readyState,
      probePhase: 'complete',
      probeCompletedAt: Date.now() - 2 * 60_000,
      probeResult: {
        uid: 'built-in-1',
        name: 'MacBook Pro 麦克风',
        transport: 'built_in',
        input_sample_rate_hz: 44_100,
        input_channel_count: 1,
        read_back_uid: 'built-in-1',
        first_frame_sample_count: 128,
        elapsed_ms: 12,
        result: 'compatible',
      },
    }, { onProbe: probe })

    await user.click(screen.getByRole('button', { name: '重新测试所选麦克风' }))

    expect(probe).toHaveBeenCalledOnce()
    expect(screen.getByRole('status')).toHaveTextContent('内置 · 已通过测试 · 44.1 kHz · 1 声道 · 2 分钟前测试')
  })

  it('does not show a cached probe as ready when the saved device is disconnected', () => {
    renderSelector({
      ...readyState,
      availability: 'disconnected',
      devices: [],
      probePhase: 'complete',
      probeResult: {
        uid: 'built-in-1', name: 'MacBook Pro 麦克风', transport: 'built_in',
        input_sample_rate_hz: 44_100, input_channel_count: 1, read_back_uid: 'built-in-1',
        first_frame_sample_count: 128, elapsed_ms: 12, result: 'compatible',
      },
    })
    expect(screen.getByRole('status')).toHaveTextContent('已断开')
    expect(screen.getByRole('status')).not.toHaveTextContent('已通过测试')
  })
})

function renderSelector(
  state: MicrophoneDeviceState,
  overrides: Partial<React.ComponentProps<typeof MicrophoneDeviceSelector>> = {},
) {
  render(
    <MicrophoneDeviceSelector
      state={state}
      locked={false}
      onSelect={vi.fn()}
      onRefresh={vi.fn()}
      {...overrides}
    />,
  )
}
