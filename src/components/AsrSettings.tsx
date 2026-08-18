import { useState, useEffect } from 'react'
import { loadVoiceprints, loadAsrConfig } from '../data/adapter'
import type { CoreVoiceprint, CoreAsrConfig } from '../services/lifesub'

export function AsrSettings() {
  const [voiceprints, setVoiceprints] = useState<CoreVoiceprint[]>([])
  const [config, setConfig] = useState<CoreAsrConfig | null>(null)

  useEffect(() => {
    loadVoiceprints().then(setVoiceprints)
    loadAsrConfig().then(setConfig)
  }, [])

  const providerLabel = (p: string) =>
    p === 'sense_voice' ? 'SenseVoice（推荐）' : p === 'whisper' ? 'Whisper' : p === 'qwen3_asr' ? 'Qwen3-ASR' : p

  const langLabel = (l: string) => l === 'zh' ? '中文' : l === 'en' ? 'English' : l === 'auto' ? '自动检测' : l

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ASR</span>
      <h1>ASR 设置</h1>

      <section className="settings-section">
        <h2>Provider</h2>
        <div className="setting-row">
          <label>当前 Provider</label>
          <span>{config ? providerLabel(config.provider) : '加载中...'}</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>语言与行为</h2>
        <div className="setting-row">
          <label>识别语言</label>
          <span>{config ? langLabel(config.language) : '加载中...'}</span>
        </div>
        <div className="setting-row">
          <label>自动转写</label>
          <span className={`status-pill ${config?.auto_transcribe ? '' : 'status-pill--quiet'}`}>
            {config?.auto_transcribe ? '启用' : '停用'}
          </span>
        </div>
        <div className="setting-row">
          <label>线程数</label>
          <span>{config?.threads ?? '—'}</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>VAD 设置</h2>
        <div className="setting-row">
          <label>VAD</label>
          <span className={`status-pill ${config?.vad_enabled ? '' : 'status-pill--quiet'}`}>
            {config?.vad_enabled ? '启用' : '停用'}
          </span>
        </div>
        <div className="setting-row">
          <label>最小语音长度</label>
          <span>{config?.vad_min_speech_ms ?? '—'} ms</span>
        </div>
        <div className="setting-row">
          <label>静音阈值</label>
          <span>{config?.vad_silence_ms ?? '—'} ms</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>Provider 专属选项</h2>
        <div className="setting-row">
          <label>ITN（逆文本正则化）</label>
          <span className={`status-pill ${config?.itn_enabled ? '' : 'status-pill--quiet'}`}>
            {config?.itn_enabled ? '启用' : '停用'}
          </span>
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
    </div>
  )
}