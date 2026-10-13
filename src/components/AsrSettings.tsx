import { useEffect, useMemo, useState } from 'react'
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

const PROVIDERS: Array<{ value: string; label: string }> = [
  { value: 'sense_voice', label: 'SenseVoice（推荐）' },
  { value: 'whisper', label: 'Whisper' },
  { value: 'qwen3_asr', label: 'Qwen3-ASR' },
]

const LANGUAGE_LABELS: Record<string, string> = {
  auto: '自动检测',
  zh: '中文',
  en: 'English',
  ja: '日本語',
  ko: '한국어',
  yue: '粤语',
}

const WHISPER_TASK_OPTIONS: Array<{ value: 'transcribe' | 'translate'; label: string }> = [
  { value: 'transcribe', label: '转写原语言' },
  { value: 'translate', label: '翻译为英语' },
]

interface AsrSettingsProps {
  onDirtyChange?: (dirty: boolean) => void
}

export function AsrSettings({ onDirtyChange }: AsrSettingsProps = {}) {
  const [voiceprints, setVoiceprints] = useState<CoreVoiceprint[]>([])
  const [config, setConfig] = useState<CoreAsrConfig | null>(null)
  const [savedConfig, setSavedConfig] = useState<CoreAsrConfig | null>(null)
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

    const loadedModels = modelResult.status === 'fulfilled' && Array.isArray(modelResult.value)
      ? modelResult.value
      : []

    setModels(loadedModels)
    const loadedConfig = configResult.status === 'fulfilled'
      ? normalizeAsrConfig(configResult.value, loadedModels)
      : null
    setConfig(loadedConfig)
    setSavedConfig(loadedConfig)
    setVoiceprints(voiceprintResult.status === 'fulfilled' ? voiceprintResult.value : [])

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

  const updateConfig = (updater: (current: CoreAsrConfig) => CoreAsrConfig) => {
    setConfig((current) => current ? normalizeAsrConfig(updater(current), models) : null)
  }

  const handleSave = async () => {
    if (!config || loadError || loading) return
    setSaveError(null)
    setStatusMessage(null)
    try {
      const normalized = normalizeAsrConfig(config, models)
      await saveAsrConfig(normalized)
      setConfig(normalized)
      setSavedConfig(normalized)
      setStatusMessage('设置已保存')
    } catch (error) {
      setSaveError(errorText(error))
    }
  }

  const set = <K extends keyof CoreAsrConfig>(key: K, value: CoreAsrConfig[K]) => {
    updateConfig((current) => ({ ...current, [key]: value }))
  }

  const setProvider = (provider: string) => {
    updateConfig((current) => ({
      ...current,
      provider,
      model_id: '',
      itn_enabled: provider === 'sense_voice' ? current.itn_enabled : false,
      whisper_task: provider === 'whisper' ? normalizeWhisperTask(current.whisper_task) : null,
    }))
  }

  const [editingVoiceprintId, setEditingVoiceprintId] = useState<string | null>(null)
  const [renameDraft, setRenameDraft] = useState('')
  const [deleteVoiceprintId, setDeleteVoiceprintId] = useState<string | null>(null)

  const beginRenameVoiceprint = (voiceprint: CoreVoiceprint) => {
    setEditingVoiceprintId(voiceprint.id)
    setRenameDraft(voiceprint.name)
    setDeleteVoiceprintId(null)
  }

  const handleRenameVoiceprint = async (voiceprint: CoreVoiceprint) => {
    const nextName = renameDraft.trim()
    if (!nextName || nextName === voiceprint.name) return

    setPendingMutation({ action: 'rename', id: voiceprint.id })
    setMutationError(null)
    try {
      await renameVoiceprintAdapter(voiceprint.id, nextName)
      setVoiceprints((previous) => previous.map((item) => item.id === voiceprint.id ? { ...item, name: nextName } : item))
      setEditingVoiceprintId(null)
      setStatusMessage('声纹已重命名')
    } catch (error) {
      setMutationError(errorText(error))
    } finally {
      setPendingMutation(null)
    }
  }

  const handleDeleteVoiceprint = async (voiceprint: CoreVoiceprint) => {
    setPendingMutation({ action: 'delete', id: voiceprint.id })
    setMutationError(null)
    try {
      await deleteVoiceprintAdapter(voiceprint.id)
      setVoiceprints((previous) => previous.filter((item) => item.id !== voiceprint.id))
      setDeleteVoiceprintId(null)
      setStatusMessage('声纹已删除')
    } catch (error) {
      setMutationError(errorText(error))
    } finally {
      setPendingMutation(null)
    }
  }

  const providerModels = config ? models.filter((model) => model.provider === config.provider) : []
  const selectedModel = config ? findSelectedModel(models, config.provider, config.model_id) : null
  const languageOptions = selectedModel?.supported_languages ?? []
  const controlsDisabled = loading || Boolean(loadError) || !config || Boolean(pendingMutation)
  const dirty = useMemo(() => Boolean(config && savedConfig && JSON.stringify(config) !== JSON.stringify(savedConfig)), [config, savedConfig])
  const saveDisabled = controlsDisabled || !config?.model_id || !dirty
  const status = mutationError ?? saveError ?? statusMessage

  useEffect(() => {
    onDirtyChange?.(dirty)
  }, [dirty, onDirtyChange])

  return (
    <div className="settings-tab-content">
      <h1>ASR 设置</h1>

      <div className="settings-readiness">
        <strong>转写准备</strong>
        <span>{asrReadiness(loading, loadError, config, selectedModel)}</span>
      </div>

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
              {PROVIDERS.map((provider) => <option key={provider.value} value={provider.value}>{provider.label}</option>)}
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
              {providerModels.map((model) => (
                <option key={model.model_id} value={model.model_id}>{model.display_name}</option>
              ))}
            </select>
          ) : <span>{loading ? '加载中...' : '—'}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>语言与行为</h2>
        <div className="setting-row">
          <label htmlFor="asr-language">识别语言</label>
          {config && selectedModel ? (
            <select id="asr-language" className="dictionary-view__scope" value={config.language} onChange={(e) => set('language', e.target.value)} disabled={controlsDisabled}>
              {languageOptions.map((language) => <option key={language} value={language}>{languageLabel(language)}</option>)}
            </select>
          ) : <span>{config ? '请先选择模型' : loading ? '加载中...' : '—'}</span>}
        </div>
        <div className="setting-row">
          <label>自动转写</label>
          <label className="toggle">
            <input type="checkbox" aria-label="自动转写" checked={config?.auto_transcribe ?? false} onChange={(e) => set('auto_transcribe', e.target.checked)} disabled={controlsDisabled} />
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
            <input type="checkbox" aria-label="VAD" checked={config?.vad_enabled ?? false} onChange={(e) => set('vad_enabled', e.target.checked)} disabled={controlsDisabled} />
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

      {config?.provider === 'sense_voice' && (
        <section className="settings-section">
          <h2>Provider 专属选项</h2>
          <div className="setting-row">
            <label>ITN（逆文本正则化）</label>
            <label className="toggle">
              <input type="checkbox" aria-label="ITN（逆文本正则化）" checked={config.itn_enabled} onChange={(e) => set('itn_enabled', e.target.checked)} disabled={controlsDisabled} />
              <span className={`status-pill ${config.itn_enabled ? '' : 'status-pill--quiet'}`}>
                {config.itn_enabled ? '启用' : '停用'}
              </span>
            </label>
          </div>
        </section>
      )}

      {config?.provider === 'whisper' && (
        <section className="settings-section">
          <h2>Provider 专属选项</h2>
          <div className="setting-row">
            <label htmlFor="whisper-task">Whisper 任务</label>
            <select
              id="whisper-task"
              className="dictionary-view__scope"
              value={normalizeWhisperTask(config.whisper_task)}
              onChange={(e) => set('whisper_task', e.target.value as CoreAsrConfig['whisper_task'])}
              disabled={controlsDisabled}
            >
              {WHISPER_TASK_OPTIONS.map((task) => (
                <option key={task.value} value={task.value}>{task.label}</option>
              ))}
            </select>
          </div>
        </section>
      )}

      <section className="settings-section">
        <h2>声纹库</h2>
        <p>这里管理已有声纹记录；自动识别说话人尚未接入当前转写流程。</p>
        {voiceprints.length === 0 ? (
          <p className="empty-state">暂无已注册声纹；当前录音不会自动创建声纹。</p>
        ) : (
          <div className="voiceprint-list">
            {voiceprints.map((vp) => {
              const isPending = pendingMutation?.id === vp.id
              const disableActions = pendingMutation !== null
              return (
                <div key={vp.id} className="voiceprint-card">
                  <div className="voiceprint-card__info">
                    {editingVoiceprintId === vp.id ? (
                      <label className="voiceprint-card__editor">
                        <span>声纹名称</span>
                        <input className="setting-input" aria-label="声纹名称" value={renameDraft} onChange={(event) => setRenameDraft(event.target.value)} disabled={isPending} />
                      </label>
                    ) : <strong>{vp.name}</strong>}
                    <small>{vp.sample_count} 个样本 · 更新于 {vp.updated_at?.slice(0, 10) ?? '—'}</small>
                    {vp.dictionary_entry_id && <span className="status-pill">关联词典</span>}
                  </div>
                  <div className="voiceprint-card__actions">
                    {editingVoiceprintId === vp.id ? (
                      <>
                        <button type="button" className="text-button" onClick={() => void handleRenameVoiceprint(vp)} disabled={disableActions || !renameDraft.trim() || renameDraft.trim() === vp.name}>
                          {isPending && pendingMutation?.action === 'rename' ? '重命名中…' : '保存重命名'}
                        </button>
                        <button type="button" className="text-button" onClick={() => setEditingVoiceprintId(null)} disabled={disableActions}>取消重命名</button>
                      </>
                    ) : deleteVoiceprintId === vp.id ? (
                      <div className="voiceprint-card__confirm" role="group" aria-label={`删除 ${vp.name}`}>
                        <span>确认删除“{vp.name}”？</span>
                        <button type="button" className="text-button" onClick={() => void handleDeleteVoiceprint(vp)} disabled={disableActions}>
                          {isPending && pendingMutation?.action === 'delete' ? '删除中…' : '确认删除'}
                        </button>
                        <button type="button" className="text-button" onClick={() => setDeleteVoiceprintId(null)} disabled={disableActions}>取消删除</button>
                      </div>
                    ) : (
                      <>
                        <button type="button" className="text-button" onClick={() => beginRenameVoiceprint(vp)} disabled={disableActions}>重命名</button>
                        <button type="button" className="text-button" onClick={() => { setDeleteVoiceprintId(vp.id); setEditingVoiceprintId(null) }} disabled={disableActions}>删除</button>
                      </>
                    )}
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
      {dirty && <p className="settings-dirty-indicator" role="status">有未保存的更改</p>}
      {status && (
        <p className={`settings-feedback ${loadError || mutationError || saveError ? 'settings-feedback--error' : ''}`} role="status">
          {status}
        </p>
      )}
    </div>
  )
}

function asrReadiness(
  loading: boolean,
  loadError: string | null,
  config: CoreAsrConfig | null,
  selectedModel: CoreAsrModel | null,
) {
  if (loading) return '正在读取 ASR 设置。'
  if (loadError || !config) return '设置未读取成功，请重试。'
  if (!config.auto_transcribe) return '自动转写已关闭。'
  if (!selectedModel) return '请选择转写模型。'
  if (selectedModel.installation_state !== 'runtime_qualified' || !selectedModel.executable) {
    return '设置载入时该模型尚未就绪；安装后请刷新模型状态。'
  }
  return '设置载入时该模型已通过运行时验证；执行时仍会重新核验。'
}

function findSelectedModel(models: CoreAsrModel[], provider: string, modelId: string) {
  return models.find((model) => model.provider === provider && model.model_id === modelId) ?? null
}

function normalizeAsrConfig(config: CoreAsrConfig, models: CoreAsrModel[]): CoreAsrConfig {
  const selectedModel = findSelectedModel(models, config.provider, config.model_id)
  const supportedLanguages = selectedModel?.supported_languages ?? []

  return {
    ...config,
    language: supportedLanguages.length > 0
      ? normalizeLanguage(config.language, supportedLanguages)
      : config.language,
    itn_enabled: config.provider === 'sense_voice' ? config.itn_enabled : false,
    whisper_task: config.provider === 'whisper' ? normalizeWhisperTask(config.whisper_task) : null,
  }
}

function normalizeLanguage(language: string, supportedLanguages: string[]) {
  if (supportedLanguages.includes(language)) return language
  if (supportedLanguages.includes('auto')) return 'auto'
  return supportedLanguages[0] ?? language
}

function normalizeWhisperTask(task: CoreAsrConfig['whisper_task']) {
  return task === 'translate' ? 'translate' : 'transcribe'
}

function languageLabel(language: string) {
  return LANGUAGE_LABELS[language] ?? language
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '操作失败'
}
