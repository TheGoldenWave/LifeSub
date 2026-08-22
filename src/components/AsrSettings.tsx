import { useEffect, useState } from 'react'
import {
  deleteVoiceprintAdapter,
  loadAsrConfig,
  loadModelCatalog,
  loadVoiceprints,
  renameVoiceprintAdapter,
  saveAsrConfig,
} from '../data/adapter'
import type { CoreAsrConfig, CoreAsrModel, CoreVoiceprint } from '../services/lifesub'

type VoiceprintMutation =
  | { action: 'rename'; id: string }
  | { action: 'delete'; id: string }
  | null

export function AsrSettings() {
  const [voiceprints, setVoiceprints] = useState<CoreVoiceprint[]>([])
  const [config, setConfig] = useState<CoreAsrConfig | null>(null)
  const [models, setModels] = useState<CoreAsrModel[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [statusMessage, setStatusMessage] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [pendingMutation, setPendingMutation] = useState<VoiceprintMutation>(null)

  useEffect(() => {
    void reload()
  }, [])

  const reload = async () => {
    setLoading(true)
    setLoadError(null)
    setMutationError(null)

    const [configResult, voiceprintResult, modelResult] = await Promise.allSettled([
      loadAsrConfig(),
      loadVoiceprints(),
      loadModelCatalog(),
    ])

    if (configResult.status === 'fulfilled') {
      setConfig(configResult.value)
    } else {
      setConfig(null)
    }

    if (voiceprintResult.status === 'fulfilled') {
      setVoiceprints(voiceprintResult.value)
    } else {
      setVoiceprints([])
    }

    if (modelResult.status === 'fulfilled') {
      setModels(Array.isArray(modelResult.value) ? modelResult.value : [])
    } else {
      setModels([])
    }

    const errorMessage = configResult.status === 'rejected'
      ? errorText(configResult.reason)
      : voiceprintResult.status === 'rejected'
        ? errorText(voiceprintResult.reason)
        : modelResult.status === 'rejected'
          ? errorText(modelResult.reason)
        : null

    setLoadError(errorMessage)
    setLoading(false)
  }

  const handleSave = async () => {
    if (!config || loadError || loading) return
    setSaveError(null)
    setStatusMessage(null)
    try {
      await saveAsrConfig(config)
      setStatusMessage('设置已保存')
    } catch (error) {
      setSaveError(errorText(error))
    }
  }

  const set = <K extends keyof CoreAsrConfig>(key: K, value: CoreAsrConfig[K]) => {
    setConfig((prev) => prev ? { ...prev, [key]: value } : null)
  }

  const setProvider = (provider: string) => {
    setConfig((previous) => previous ? {
      ...previous,
      provider,
      model_id: '',
    } : null)
  }

  const handleRenameVoiceprint = async (voiceprint: CoreVoiceprint) => {
    const nextName = window.prompt('输入新的声纹名称', voiceprint.name)?.trim()
    if (!nextName || nextName === voiceprint.name) return

    setPendingMutation({ action: 'rename', id: voiceprint.id })
    setMutationError(null)
    try {
      await renameVoiceprintAdapter(voiceprint.id, nextName)
      setVoiceprints((previous) => previous.map((item) => item.id === voiceprint.id ? { ...item, name: nextName } : item))
      setStatusMessage('声纹已重命名')
    } catch (error) {
      setMutationError(errorText(error))
    } finally {
      setPendingMutation(null)
    }
  }

  const handleDeleteVoiceprint = async (voiceprint: CoreVoiceprint) => {
    if (!window.confirm(`删除声纹“${voiceprint.name}”？`)) return

    setPendingMutation({ action: 'delete', id: voiceprint.id })
    setMutationError(null)
    try {
      await deleteVoiceprintAdapter(voiceprint.id)
      setVoiceprints((previous) => previous.filter((item) => item.id !== voiceprint.id))
      setStatusMessage('声纹已删除')
    } catch (error) {
      setMutationError(errorText(error))
    } finally {
      setPendingMutation(null)
    }
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

  const controlsDisabled = loading || Boolean(loadError) || !config || Boolean(pendingMutation)
  const saveDisabled = controlsDisabled || !config?.model_id
  const status = mutationError ?? saveError ?? statusMessage

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ASR</span>
      <h1>ASR 设置</h1>

      {loadError && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{loadError}</p>
          <button type="button" className="button" onClick={() => void reload()}>
            重试加载 ASR 设置
          </button>
        </div>
      )}

      <section className="settings-section">
        <h2>Provider</h2>
        <div className="setting-row">
          <label htmlFor="asr-provider">当前 Provider</label>
          {config ? (
            <select id="asr-provider" className="dictionary-view__scope" value={config.provider} onChange={(e) => setProvider(e.target.value)} disabled={controlsDisabled}>
              {providers.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label htmlFor="asr-model">当前模型</label>
          {config ? (
            <select
              id="asr-model"
              className="dictionary-view__scope"
              value={config.model_id}
              onChange={(event) => set('model_id', event.target.value)}
              disabled={controlsDisabled}
            >
              <option value="" disabled>请选择模型</option>
              {models.filter((model) => model.provider === config.provider).map((model) => (
                <option key={model.model_id} value={model.model_id}>{model.display_name}</option>
              ))}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>语言与行为</h2>
        <div className="setting-row">
          <label>识别语言</label>
          {config ? (
            <select className="dictionary-view__scope" value={config.language} onChange={(e) => set('language', e.target.value)} disabled={controlsDisabled}>
              {languages.map((l) => <option key={l.value} value={l.value}>{l.label}</option>)}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label>自动转写</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.auto_transcribe ?? false} onChange={(e) => set('auto_transcribe', e.target.checked)} disabled={controlsDisabled} />
            <span className={`status-pill ${config?.auto_transcribe ? '' : 'status-pill--quiet'}`}>
              {config?.auto_transcribe ? '启用' : '停用'}
            </span>
          </label>
        </div>
        <div className="setting-row">
          <label>线程数</label>
          {config ? (
            <input type="number" className="setting-input" min={1} max={16} value={config.threads} onChange={(e) => set('threads', Number(e.target.value))} disabled={controlsDisabled} />
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>VAD 设置</h2>
        <div className="setting-row">
          <label>VAD</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.vad_enabled ?? false} onChange={(e) => set('vad_enabled', e.target.checked)} disabled={controlsDisabled} />
            <span className={`status-pill ${config?.vad_enabled ? '' : 'status-pill--quiet'}`}>
              {config?.vad_enabled ? '启用' : '停用'}
            </span>
          </label>
        </div>
        <div className="setting-row">
          <label>最小语音长度 (ms)</label>
          {config ? (
            <input type="number" className="setting-input" min={100} max={5000} step={100} value={config.vad_min_speech_ms} onChange={(e) => set('vad_min_speech_ms', Number(e.target.value))} disabled={controlsDisabled} />
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label>静音阈值 (ms)</label>
          {config ? (
            <input type="number" className="setting-input" min={200} max={10000} step={100} value={config.vad_silence_ms} onChange={(e) => set('vad_silence_ms', Number(e.target.value))} disabled={controlsDisabled} />
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>Provider 专属选项</h2>
        <div className="setting-row">
          <label>ITN（逆文本正则化）</label>
          <label className="toggle">
            <input type="checkbox" checked={config?.itn_enabled ?? false} onChange={(e) => set('itn_enabled', e.target.checked)} disabled={controlsDisabled} />
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
            {voiceprints.map((vp) => {
              const isPending = pendingMutation?.id === vp.id
              const disableActions = pendingMutation !== null
              return (
                <div key={vp.id} className="voiceprint-card">
                  <div className="voiceprint-card__info">
                    <strong>{vp.name}</strong>
                    <small>{vp.sample_count} 个样本 · 更新于 {vp.updated_at?.slice(0, 10) ?? '—'}</small>
                    {vp.dictionary_entry_id && <span className="status-pill">关联词典</span>}
                  </div>
                  <div className="voiceprint-card__actions">
                    <button
                      type="button"
                      className="text-button"
                      onClick={() => void handleRenameVoiceprint(vp)}
                      disabled={disableActions}
                    >
                      {isPending && pendingMutation?.action === 'rename' ? '重命名中…' : '重命名'}
                    </button>
                    <button
                      type="button"
                      className="text-button"
                      onClick={() => void handleDeleteVoiceprint(vp)}
                      disabled={disableActions}
                    >
                      {isPending && pendingMutation?.action === 'delete' ? '删除中…' : '删除'}
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
        <button type="button" className="text-button settings-planned-button" disabled aria-disabled="true">
          + 注册新声纹（计划中）
        </button>
      </section>

      <div className="settings-actions">
        <button className="button button--primary" onClick={handleSave} disabled={saveDisabled}>
          保存设置
        </button>
      </div>
      {status && (
        <p className={`settings-feedback ${loadError || mutationError || saveError ? 'settings-feedback--error' : ''}`} role="status">
          {status}
        </p>
      )}
    </div>
  )
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '操作失败'
}
