import { useEffect, useState } from 'react'
import { loadRuntimeInfo } from '../data/adapter'
import type { AppRuntimeInfo } from '../services/lifesub'

export function AboutTab() {
  const [info, setInfo] = useState<AppRuntimeInfo | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void reload()
  }, [])

  const reload = async () => {
    setLoading(true)
    setError(null)
    try {
      setInfo(await loadRuntimeInfo())
    } catch (cause) {
      setInfo(null)
      setError(errorText(cause))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ABOUT</span>
      <h1>关于 LifeSub</h1>

      {error && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{error}</p>
          <button type="button" className="button" onClick={() => void reload()}>
            重试加载运行时信息
          </button>
        </div>
      )}

      <section className="settings-section">
        <div className="setting-row">
          <label>版本</label>
          <span>{info?.app_version ?? (loading ? '加载中...' : '—')}</span>
        </div>
        <div className="setting-row">
          <label>Tauri</label>
          <span>{info?.tauri_version ?? (loading ? '加载中...' : '—')}</span>
        </div>
        <div className="setting-row">
          <label>前端</label>
          <span>{info?.frontend_stack ?? (loading ? '加载中...' : '—')}</span>
        </div>
        <div className="setting-row">
          <label>ASR 运行时</label>
          <span>{info?.asr_runtime ?? (loading ? '加载中...' : '—')}</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>本地优先承诺</h2>
        <p>所有录音、转写与声纹数据均存储在本机，云端处理默认关闭。</p>
      </section>
    </div>
  )
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '加载失败'
}
