import { AudioLines, Mic, RefreshCw } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { MicrophoneDeviceState } from '../microphoneDevices'

const PROBE_AGE_REFRESH_MS = 30_000
const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS

interface MicrophoneDeviceSelectorProps {
  state: MicrophoneDeviceState
  locked: boolean
  onSelect: (uid: string | null) => void
  onRefresh: () => void
  onProbe?: () => void
  compact?: boolean
}

export function MicrophoneDeviceSelector({
  state,
  locked,
  onSelect,
  onRefresh,
  onProbe,
  compact = false,
}: MicrophoneDeviceSelectorProps) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!state.probeCompletedAt) return
    const interval = window.setInterval(() => setNow(Date.now()), PROBE_AGE_REFRESH_MS)
    return () => window.clearInterval(interval)
  }, [state.probeCompletedAt])
  const disconnectedSelection = state.availability === 'disconnected' && state.selectedUid
    ? { uid: state.selectedUid, name: state.selectedName ?? '已保存的麦克风' }
    : null
  const connectedDevices = state.devices.filter((device) => device.isConnected)
  const selectDisabled = locked
    || state.availability === 'loading'
    || state.availability === 'empty'
    || state.availability === 'error'
    || state.saving
  const value = state.selectedUid ?? ''

  return (
    <div className={`microphone-selector ${compact ? 'microphone-selector--compact' : ''}`}>
      <label className="microphone-selector__label" htmlFor={compact ? 'live-microphone-device' : 'settings-microphone-device'}>
        <Mic size={14} aria-hidden="true" />
        收音麦克风
      </label>
      <div className="microphone-selector__control">
        <select
          id={compact ? 'live-microphone-device' : 'settings-microphone-device'}
          className="microphone-selector__select"
          aria-label="收音麦克风"
          value={value}
          disabled={selectDisabled}
          onChange={(event) => onSelect(event.target.value || null)}
        >
          {state.availability === 'loading' && <option value="">正在检测麦克风</option>}
          {state.availability === 'empty' && <option value="">未检测到可用麦克风</option>}
          {state.availability === 'error' && <option value="">无法读取麦克风设备</option>}
          {(state.availability === 'ready' || state.availability === 'unselected') && (
            <option value="">请选择麦克风</option>
          )}
          {disconnectedSelection && (
            <option value={disconnectedSelection.uid} disabled>
              {disconnectedSelection.name}（已断开）
            </option>
          )}
          {connectedDevices.map((device) => (
            <option key={device.uid} value={device.uid}>
              {device.name}{device.isDefault ? '（系统默认）' : ''}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="icon-button microphone-selector__refresh"
          aria-label="刷新麦克风列表"
          title="刷新麦克风列表"
          disabled={locked || state.refreshing}
          onClick={onRefresh}
        >
          <RefreshCw size={15} className={state.refreshing ? 'microphone-selector__refresh-icon--active' : ''} />
        </button>
        {onProbe && (
          <button
            type="button"
            className="icon-button microphone-selector__probe"
            aria-label={state.probeResult ? '重新测试所选麦克风' : '测试所选麦克风'}
            title={state.probeResult ? '重新测试所选麦克风' : '测试所选麦克风'}
            disabled={locked || state.probePhase === 'probing' || state.availability !== 'ready' || state.saving || state.refreshing}
            onClick={onProbe}
          >
            <AudioLines size={15} aria-hidden="true" />
          </button>
        )}
      </div>
      <span className={`microphone-selector__status microphone-selector__status--${state.availability}`} role="status">
        {statusText(state, locked, now)}
      </span>
    </div>
  )
}

function statusText(state: MicrophoneDeviceState, locked: boolean, now: number) {
  if (locked) return '录制中不可切换'
  if (state.saving) return '正在保存选择'
  if (state.refreshing) return '正在刷新设备列表'
  if (state.availability !== 'ready') return availabilityText(state)
  if (state.probePhase === 'probing') return '正在测试麦克风'
  if (state.probePhase === 'error') return state.probeError ?? '麦克风测试失败'
  if (state.probeResult) {
    const age = state.probeCompletedAt ? ` · ${probeAge(now, state.probeCompletedAt)}` : ''
    return `${transportLabel(state.probeResult.transport)} · ${probeStatusText(state.probeResult)}${age}`
  }
  return availabilityText(state)
}

function availabilityText(state: MicrophoneDeviceState) {
  switch (state.availability) {
    case 'loading': return '正在检测麦克风'
    case 'empty': return '未检测到可用麦克风'
    case 'error': return state.error ?? '无法读取麦克风设备'
    case 'unselected': return '录制前请选择一个麦克风'
    case 'disconnected': return `${state.selectedName ?? '已保存的麦克风'} 已断开`
    case 'ready': {
      const selected = state.devices.find((device) => device.uid === state.selectedUid)
      return selected ? `${transportLabel(selected.transport)} · 已连接` : '已连接'
    }
  }
}

function probeAge(now: number, completedAt: number) {
  const elapsed = Math.max(0, now - completedAt)
  if (elapsed < MINUTE_MS) return '刚刚测试'
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} 分钟前测试`
  return `${Math.floor(elapsed / HOUR_MS)} 小时前测试`
}

function probeStatusText(probe: NonNullable<MicrophoneDeviceState['probeResult']>) {
  switch (probe.result) {
    case 'compatible': {
      const rate = probe.input_sample_rate_hz
        ? `${formatRate(probe.input_sample_rate_hz)} · `
        : ''
      const channels = probe.input_channel_count ? `${probe.input_channel_count} 声道` : '声道未知'
      return `已通过测试 · ${rate}${channels}`
    }
    case 'permission_denied': return '麦克风权限未授权'
    case 'unavailable': return '所选麦克风当前不可用'
    case 'identity_mismatch': return '实际输入设备与选择不一致'
    case 'invalid_format': return '麦克风音频格式不可用'
    case 'no_audio_frames': return '未检测到麦克风音频'
    case 'route_conflict': return '麦克风路由冲突'
    case 'timeout': return '麦克风测试超时'
  }
}

function formatRate(rate: number) {
  const kilohertz = rate / 1_000
  return `${Number.isInteger(kilohertz) ? kilohertz : kilohertz.toFixed(1)} kHz`
}

function transportLabel(transport: MicrophoneDeviceState['devices'][number]['transport']) {
  switch (transport) {
    case 'built_in': return '内置'
    case 'usb': return 'USB'
    case 'bluetooth': return '蓝牙'
    case 'aggregate': return '聚合设备'
    case 'virtual': return '虚拟设备'
    case 'unknown': return '外接设备'
  }
}
