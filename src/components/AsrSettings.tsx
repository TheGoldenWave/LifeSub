import { useState, useEffect } from 'react'
import { loadVoiceprints, loadAsrConfig, saveAsrConfig } from '../data/adapter'
import type { CoreVoiceprint, CoreAsrConfig } from '../services/lifesub'

export function AsrSettings() {
  const [voiceprints, setVoiceprints] = useState<CoreVoiceprint[]>([])
  const [config, setConfig] = useState<CoreAsrConfig | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    loadVoiceprints().then(setVoiceprints)
    loadAsrConfig().then(setConfig)
  }, [])

  const handleSave = async () => {
    if (!config) return
    await saveAsrConfig(config)
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  const set = <K extends keyof CoreAsrConfig>(key: K, value: CoreAsrConfig[K]) => {
    setConfig((prev) => prev ? { ...prev, [key]: value } : null)
  }

  const providers: { value: string; label: string }[] = [
    { value: 'sense_voice', label: 'SenseVoice（推荐）' },
    { value: 'whisper', label: 'Whisper' },
    { value: 'qwen3_asr', label: 'Qwen3-ASR' },
  ]

  const languages: { value: string; label: string }[] = [
    { value: 'zh', label: '中文' },
    { value: 'en', label: 'English' },
    { value: 'auto', label: '自动检测' },
  ]

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ASR</span>
      <h1>ASR 设置</h1>

      <section className="settings-section">
        <h2>Provider</h2>
        <div className="setting-row">
          <label>当前 Provider</label>
          {config ? (
            <select className="dictionary-view__scope" value={config.provider} onChange={(e) => set('provider', e.target.value)}>
              {providers.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
            </select>
          ) : <span>加载中...</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>语言与行为</h2>
        <div className="setting-row">
          <label>识别语言</label>
          {config ? (
            <select className="dictionary-view__scope" value={config.language} onChange={(e) => set('language', e.target.value)}>
              {languages.map((l) => <option key={l.value} value={l.value}>{l.label}</option>)}
            </select>
          ) : <span>加载中...</span>}
        </div>
        <div className="setting-row">
          <label>自动转写</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.auto_transcribe ?? false} onChange={(e) => set('auto_transcribe', e.target.checked)} />
            <span className={`status-pill ${config?.auto_transcribe ? '' : 'status-pill--quiet'}`}>
              {config?.auto_transcribe ? '启用' : '停用'}
            </span>
          </label>
        </div>
        <div className="setting-row">
          <label>线程数</label>
          {config ? (
            <input type="number" className="setting-input" min={1} max={16} value={config.threads} onChange={(e) => set('threads', Number(e.target.value))} />
          ) : <span>—</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>VAD 设置</h2>
        <div className="setting-row">
          <label>VAD</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.vad_enabled ?? false} onChange={(e) => set('vad_enabled', e.target.checked)} />
            <span className={`status-pill ${config?.vad_enabled ? '' : 'status-pill--quiet'}`}>
              {config?.vad_enabled ? '启用' : '停用'}
            </span>
          </label>
        </div>
        <div className="setting-row">
          <label>最小语音长度 (ms)</label>
          {config ? (
            <input type="number" className="setting-input" min={100} max={5000} step={100} value={config.vad_min_speech_ms} onChange={(e) => set('vad_min_speech_ms', Number(e.target.value))} />
          ) : <span>—</span>}
        </div>
        <div className="setting-row">
          <label>静音阈值 (ms)</label>
          {config ? (
            <input type="number" className="setting-input" min={200} max={10000} step={100} value={config.vad_silence_ms} onChange={(e) => set('vad_silence_ms', Number(e.target.value))} />
          ) : <span>—</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>Provider 专属选项</h2>
        <div className="setting-row">
          <label>ITN（逆文本正则化）</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.itn_enabled ?? false} onChange={(e) => set('itn_enabled', e.target.checked)} />
            <span className={`status-pill ${config?.itn_enabled ? '' : 'status-pill--quiet'}`}>
              {config?.itn_enabled ? '启用' : '停用'}
            </span>
          </label>
        </div>
      </section>

      <section className="settings-section">
        <h2>声纹库</h2>
        <p>已注册的说话人声纹，用于自动识别转写中的说话人。</p>
        {voiceprints.length === 0 ? (
          <p className="empty-state">暂无注册声纹，录音时点击未知说话人即可保存。</p>
        ) : (
          <div className="voiceprint-list">
            {voiceprints.map((vp) => (
              <div key={vp.id} className="voiceprint-card">
                <div className="voiceprint-card__info">
                  <strong>{vp.name}</strong>
                  <small>{vp.sample_count} 个样本 · 更新于 {vp.updated_at?.slice(0, 10) ?? '—'}</small>
                  {vp.dictionary_entry_id && <span className="status-pill">关联词典</span>}
                </div>
                <div className="voiceprint-card__actions">
                  <button className="text-button">重命名</button>
                  <button className="text-button">删除</button>
                </div>
              </div>
            ))}
          </div>
        )}
        <button className="text-button" style={{ marginTop: 'var(--spacing-2)' }}>+ 注册新声纹</button>
      </section>

      <div className="settings-actions">
        <button className="button button--primary" onClick={handleSave}>
          {saved ? '✓ 已保存' : '保存设置'}
        </button>
      </div>
    </div>
  )
}