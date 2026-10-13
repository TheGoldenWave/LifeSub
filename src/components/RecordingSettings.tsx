import { useEffect, useMemo, useState } from 'react'
import { loadRecordingConfig, loadSettingsCapabilities, saveRecordingConfig } from '../data/adapter'
import type { CoreRecordingConfig, CoreSettingsCapabilities } from '../services/lifesub'
import { useMicrophoneDevices } from '../microphoneDevices'
import { MicrophoneDeviceSelector } from './MicrophoneDeviceSelector'

const CAPTURE_MODE_LABELS: Record<string, string> = {
  smart: '智能路由（推荐）',
  mic_only: '仅麦克风',
  system_only: '仅系统音频',
}

const IM_DETECTION_UNAVAILABLE_LABEL = '当前版本不可用'

interface RecordingSettingsProps {
  onDirtyChange?: (dirty: boolean) => void
}

export function RecordingSettings({ onDirtyChange }: RecordingSettingsProps = {}) {
  const microphone = useMicrophoneDevices()
  const [config, setConfig] = useState<CoreRecordingConfig | null>(null)
  const [savedConfig, setSavedConfig] = useState<CoreRecordingConfig | null>(null)
  const [capabilities, setCapabilities] = useState<CoreSettingsCapabilities | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [saveMessage, setSaveMessage] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  useEffect(() => {
    void reload()
  }, [])

  const reload = async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const [loadedConfig, loadedCapabilities] = await Promise.all([
        loadRecordingConfig(),
        loadSettingsCapabilities(),
      ])
      const normalized = normalizeRecordingConfig(loadedConfig, loadedCapabilities)
      setCapabilities(loadedCapabilities)
      setConfig(normalized)
      setSavedConfig(normalized)
      microphone.synchronizeConfig(normalized)
    } catch (error) {
      setCapabilities(null)
      setConfig(null)
      setSavedConfig(null)
      setLoadError(errorText(error))
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!config || !capabilities || loading || loadError) return
    setSaveError(null)
    setSaveMessage(null)
    try {
      const persisted = await loadRecordingConfig()
      const normalized = normalizeRecordingConfig({
        ...config,
        microphone_device_uid: persisted.microphone_device_uid,
        microphone_device_name: persisted.microphone_device_name,
      }, capabilities)
      await saveRecordingConfig(normalized)
      setConfig(normalized)
      setSavedConfig(normalized)
      microphone.synchronizeConfig(normalized)
      setSaveMessage('设置已保存')
    } catch (error) {
      setSaveError(errorText(error))
    }
  }

  const set = <K extends keyof CoreRecordingConfig>(key: K, value: CoreRecordingConfig[K]) => {
    setConfig((prev) => prev ? { ...prev, [key]: value } : null)
  }

  const handleMicrophoneSelect = (uid: string | null) => {
    const device = uid === null
      ? null
      : microphone.devices.find((candidate) => candidate.uid === uid && candidate.isConnected)
    if (uid !== null && !device) return
    void microphone.select(uid)
  }

  const modes = (capabilities?.supported_capture_modes ?? []).map((value) => ({
    value,
    label: CAPTURE_MODE_LABELS[value] ?? value,
  }))
  const controlsDisabled = loading || Boolean(loadError) || !config || !capabilities
  const dirty = useMemo(() => hasRecordingChanges(config, savedConfig), [config, savedConfig])
  const saveDisabled = controlsDisabled || !dirty
  const status = saveError ?? saveMessage

  useEffect(() => {
    onDirtyChange?.(dirty)
  }, [dirty, onDirtyChange])

  return (
    <div className="settings-tab-content">
      <h1>录音设置</h1>

      <div className="settings-readiness">
        <strong>录音准备</strong>
        <span>{recordingReadiness(loading, loadError, config, microphone)}</span>
      </div>

      {loadError && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{loadError}</p>
          <button type="button" className="button" onClick={() => void reload()}>
            重试加载录音设置
          </button>
        </div>
      )}

      <section className="settings-section">
        <h2>捕获模式</h2>
        <div className="setting-row">
          <label>默认模式</label>
          {config && capabilities ? (
            <select aria-label="默认模式" className="dictionary-view__scope" value={config.capture_mode} onChange={(e) => set('capture_mode', e.target.value)} disabled={controlsDisabled}>
              {modes.map((mode) => <option key={mode.value} value={mode.value}>{mode.label}</option>)}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row setting-row--microphone">
          <MicrophoneDeviceSelector
            state={microphone}
            locked={microphone.locked}
            onSelect={handleMicrophoneSelect}
            onRefresh={() => void microphone.refresh()}
            onProbe={() => void microphone.probe()}
          />
        </div>
        {capabilities && !capabilities.im_detection_available ? (
          <div className="setting-row">
            <label>IM 通话检测</label>
            <span>{IM_DETECTION_UNAVAILABLE_LABEL}</span>
          </div>
        ) : (
          <>
            <div className="setting-row">
              <label>IM 通话检测</label>
              <label className="toggle">
                <input type="checkbox" aria-label="IM 通话检测" checked={config?.im_detection_enabled ?? false} onChange={(e) => set('im_detection_enabled', e.target.checked)} disabled={controlsDisabled} />
                <span className={`status-pill ${config?.im_detection_enabled ? '' : 'status-pill--quiet'}`}>
                  {config?.im_detection_enabled ? '启用' : '停用'}
                </span>
              </label>
            </div>
            {config?.im_detection_enabled && (
              <div className="setting-row">
                <label>检测应用</label>
                <div className="im-apps-checkboxes">
                  {['wechat', 'dingtalk', 'feishu', 'teams', 'zoom', 'qq'].map((app) => (
                    <label key={app} className="toggle">
                      <input
                        type="checkbox"
                        checked={config.im_apps.includes(app)}
                        disabled={controlsDisabled}
                        onChange={(e) => {
                          const apps = e.target.checked
                            ? [...config.im_apps, app]
                            : config.im_apps.filter((item) => item !== app)
                          set('im_apps', apps)
                        }}
                      />
                      <span>{app}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
            <div className="setting-row">
              <label>检测响应时间 (秒)</label>
              {config ? (
                <input type="number" className="setting-input" min={1} max={30} value={config.detection_delay_secs} onChange={(e) => set('detection_delay_secs', Number(e.target.value))} disabled={controlsDisabled} />
              ) : <span>{loading ? '加载中...' : '—'}</span>}
            </div>
            <div className="setting-row">
              <label>通话结束恢复 (秒)</label>
              {config ? (
                <input type="number" className="setting-input" min={1} max={60} value={config.recovery_delay_secs} onChange={(e) => set('recovery_delay_secs', Number(e.target.value))} disabled={controlsDisabled} />
              ) : <span>{loading ? '加载中...' : '—'}</span>}
            </div>
          </>
        )}
      </section>

      <section className="settings-section">
        <h2>音频格式</h2>
        <div className="setting-row">
          <label>编码</label>
          <span>WAV 16-bit PCM</span>
        </div>
        <div className="setting-row">
          <label>采样率</label>
          <span>{formatSampleRate(capabilities?.production_capture_output_sample_rate_hz)}</span>
        </div>
        <div className="setting-row">
          <label>默认声道</label>
          <span>单声道（非通话）</span>
        </div>
        <div className="setting-row">
          <label>通话声道</label>
          <span>双声道（系统 + 麦克风）</span>
        </div>
        <small>双声道分轨仅用于提升 ASR 准确率，说话人标注按人名（声纹/词典/手动），不按声道。</small>
      </section>

      <section className="settings-section">
        <h2>存储</h2>
        <div className="setting-row">
          <label>录音目录</label>
          <code>{capabilities?.data_directory_path ?? (loading ? '加载中...' : '—')}</code>
        </div>
      </section>

      <div className="settings-actions">
        <button className="button button--primary" onClick={handleSave} disabled={saveDisabled}>
          保存设置
        </button>
      </div>
      {dirty && <p className="settings-dirty-indicator" role="status">有未保存的更改</p>}
      {status && (
        <p className={`settings-feedback ${loadError || saveError ? 'settings-feedback--error' : ''}`} role="status">
          {status}
        </p>
      )}
    </div>
  )
}

function recordingReadiness(
  loading: boolean,
  loadError: string | null,
  config: CoreRecordingConfig | null,
  microphone: ReturnType<typeof useMicrophoneDevices>,
) {
  if (loading) return '正在读取录音设置。'
  if (loadError || !config) return '设置未读取成功，请重试。'
  if (config.capture_mode === 'system_only') return '仅系统音频；权限将在开始记录时检查。'
  if (microphone.availability === 'disconnected') return '所选麦克风已断开，请连接后重新测试。'
  if (microphone.availability !== 'ready') return '请选择已连接的麦克风。'
  if (microphone.probeResult?.result === 'compatible') return '所选麦克风已通过首帧测试；开始记录时仍会重新检查。'
  return '所选麦克风已连接；可运行首帧测试。'
}

function hasRecordingChanges(config: CoreRecordingConfig | null, savedConfig: CoreRecordingConfig | null) {
  if (!config || !savedConfig) return false
  return JSON.stringify(editableRecordingConfig(config)) !== JSON.stringify(editableRecordingConfig(savedConfig))
}

function editableRecordingConfig(config: CoreRecordingConfig) {
  const { microphone_device_uid: _uid, microphone_device_name: _name, ...editable } = config
  return editable
}

function normalizeRecordingConfig(
  config: CoreRecordingConfig,
  capabilities: CoreSettingsCapabilities,
): CoreRecordingConfig {
  return {
    ...config,
    capture_mode: capabilities.supported_capture_modes.includes(config.capture_mode)
      ? config.capture_mode
      : 'smart',
    im_detection_enabled: capabilities.im_detection_available && config.im_detection_enabled,
    im_apps: capabilities.im_detection_available ? config.im_apps : [],
    sample_rate: capabilities.production_capture_output_sample_rate_hz,
    storage_path: capabilities.data_directory_path,
    microphone_device_uid: config.microphone_device_uid ?? null,
    microphone_device_name: config.microphone_device_name ?? null,
  }
}

function formatSampleRate(sampleRate: number | undefined) {
  if (!sampleRate) return '加载中...'
  return `${sampleRate / 1000} kHz`
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '操作失败'
}
