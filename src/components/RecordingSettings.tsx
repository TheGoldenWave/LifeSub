import { useEffect, useState } from 'react'
import { loadRecordingConfig, saveRecordingConfig } from '../data/adapter'
import type { CoreRecordingConfig } from '../services/lifesub'

export function RecordingSettings() {
  const [config, setConfig] = useState<CoreRecordingConfig | null>(null)
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
      const loaded = await loadRecordingConfig()
      setConfig(loaded)
    } catch (error) {
      setConfig(null)
      setLoadError(errorText(error))
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!config || loading || loadError) return
    setSaveError(null)
    setSaveMessage(null)
    try {
      await saveRecordingConfig(config)
      setSaveMessage('设置已保存')
    } catch (error) {
      setSaveError(errorText(error))
    }
  }

  const set = <K extends keyof CoreRecordingConfig>(key: K, value: CoreRecordingConfig[K]) => {
    setConfig((prev) => prev ? { ...prev, [key]: value } : null)
  }

  const modes: { value: string; label: string }[] = [
    { value: 'smart', label: '智能路由（推荐）' },
    { value: 'mic_only', label: '仅麦克风' },
    { value: 'system_only', label: '仅系统音频' },
  ]

  const imApps = ['wechat', 'dingtalk', 'feishu', 'teams', 'zoom', 'qq']
  const saveDisabled = loading || Boolean(loadError) || !config
  const status = saveError ?? saveMessage

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">RECORDING</span>
      <h1>录音设置</h1>

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
          {config ? (
            <select className="dictionary-view__scope" value={config.capture_mode} onChange={(e) => set('capture_mode', e.target.value)} disabled={saveDisabled}>
              {modes.map((mode) => <option key={mode.value} value={mode.value}>{mode.label}</option>)}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label>IM 通话检测</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.im_detection_enabled ?? false} onChange={(e) => set('im_detection_enabled', e.target.checked)} disabled={saveDisabled} />
            <span className={`status-pill ${config?.im_detection_enabled ? '' : 'status-pill--quiet'}`}>
              {config?.im_detection_enabled ? '启用' : '停用'}
            </span>
          </label>
        </div>
        {config?.im_detection_enabled && (
          <div className="setting-row">
            <label>检测应用</label>
            <div className="im-apps-checkboxes">
              {imApps.map((app) => (
                <label key={app} className="toggle">
                  <input
                    type="checkbox"
                    checked={config.im_apps.includes(app)}
                    disabled={saveDisabled}
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
            <input type="number" className="setting-input" min={1} max={30} value={config.detection_delay_secs} onChange={(e) => set('detection_delay_secs', Number(e.target.value))} disabled={saveDisabled} />
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label>通话结束恢复 (秒)</label>
          {config ? (
            <input type="number" className="setting-input" min={1} max={60} value={config.recovery_delay_secs} onChange={(e) => set('recovery_delay_secs', Number(e.target.value))} disabled={saveDisabled} />
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>音频格式</h2>
        <div className="setting-row">
          <label>编码</label>
          <span>WAV 16-bit PCM</span>
        </div>
        <div className="setting-row">
          <label>采样率</label>
          {config ? (
            <select className="dictionary-view__scope" value={String(config.sample_rate)} onChange={(e) => set('sample_rate', Number(e.target.value))} disabled={saveDisabled}>
              <option value="8000">8 kHz</option>
              <option value="16000">16 kHz</option>
              <option value="44100">44.1 kHz</option>
              <option value="48000">48 kHz</option>
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
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
          {config ? (
            <input type="text" className="setting-input" value={config.storage_path} onChange={(e) => set('storage_path', e.target.value)} disabled={saveDisabled} />
          ) : <code>{loading ? '加载中...' : '~/.lifesub/recordings/'}</code>}
        </div>
      </section>

      <div className="settings-actions">
        <button className="button button--primary" onClick={handleSave} disabled={saveDisabled}>
          保存设置
        </button>
      </div>
      {status && (
        <p className={`settings-feedback ${loadError || saveError ? 'settings-feedback--error' : ''}`} role="status">
          {status}
        </p>
      )}
    </div>
  )
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '操作失败'
}
