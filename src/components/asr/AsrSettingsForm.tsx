import { AlertTriangle, Loader2, Save } from 'lucide-react'
import { useCallback, useState } from 'react'
import type { AsrProviderKind, AsrSettings, ModelInfo, WhisperTask } from '../../domain'
import { ModelCardList } from './ModelCardList'
import { ProviderSelector } from './ProviderSelector'

const LANGUAGES: { value: string; label: string }[] = [
  { value: 'zh', label: '中文' },
  { value: 'en', label: 'English' },
  { value: 'ja', label: '日本語' },
  { value: 'ko', label: '한국어' },
  { value: 'yue', label: '粵語' },
  { value: 'de', label: 'Deutsch' },
  { value: 'fr', label: 'Français' },
  { value: 'es', label: 'Español' },
]

interface AsrSettingsFormProps {
  settings: AsrSettings | null
  models: ModelInfo[]
  onSave: (settings: AsrSettings) => Promise<void>
  onDownloadModel: (modelId: string) => Promise<void>
  onCancelDownload: (downloadId: string) => Promise<void>
  onDeleteModel: (modelId: string) => Promise<void>
  loading: boolean
  error: string | null
}

export function AsrSettingsForm({
  settings,
  models,
  onSave,
  onDownloadModel,
  onCancelDownload,
  onDeleteModel,
  loading,
  error,
}: AsrSettingsFormProps) {
  const [saving, setSaving] = useState(false)

  const handleProviderChange = useCallback(
    (provider: AsrProviderKind) => {
      if (!settings) return
      const newModel = models.find((m) => m.provider === provider)
      const providerOptions =
        provider === 'sense_voice'
          ? { kind: 'sense_voice' as const, useItn: true }
          : { kind: 'whisper' as const, task: 'transcribe' as const }
      onSave({
        ...settings,
        provider,
        modelId: newModel?.modelId ?? '',
        providerOptions,
      })
    },
    [settings, models, onSave],
  )

  const handleModelSelect = useCallback(
    (modelId: string) => {
      if (!settings) return
      onSave({ ...settings, modelId })
    },
    [settings, onSave],
  )

  const handleSave = useCallback(async () => {
    if (!settings) return
    setSaving(true)
    try {
      await onSave(settings)
    } finally {
      setSaving(false)
    }
  }, [settings, onSave])

  if (loading) {
    return (
      <div className="asr-settings-form__loading" role="status" aria-label="Loading ASR settings">
        <Loader2 size={24} className="asr-settings-form__spinner" />
        <span>加载 ASR 设置...</span>
      </div>
    )
  }

  if (!settings) {
    return (
      <div className="asr-settings-form__empty">
        <AlertTriangle size={20} />
        <span>无法加载 ASR 设置。</span>
      </div>
    )
  }

  const compatibleModels = models.filter((m) => m.provider === settings.provider)
  const isSenseVoice = settings.provider === 'sense_voice'

  return (
    <div className="asr-settings-form">
      {error && (
        <div className="asr-settings-form__error" role="alert">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </div>
      )}

      <section className="settings-section">
        <h2>Provider</h2>
        <ProviderSelector provider={settings.provider} onChange={handleProviderChange} />
      </section>

      <section className="settings-section">
        <h2>模型</h2>
        <ModelCardList
          models={compatibleModels}
          selectedModelId={settings.modelId}
          onSelect={handleModelSelect}
          onDownload={onDownloadModel}
          onCancel={onCancelDownload}
          onDelete={onDeleteModel}
        />
      </section>

      <section className="settings-section">
        <h2>识别参数</h2>

        <div className="setting-row">
          <label htmlFor="asr-language">语言</label>
          <select
            id="asr-language"
            aria-label="Language"
            value={settings.language}
            onChange={(e) => onSave({ ...settings, language: e.target.value })}
            className="asr-settings-form__select"
          >
            {LANGUAGES.map((lang) => (
              <option key={lang.value} value={lang.value}>
                {lang.label}
              </option>
            ))}
          </select>
        </div>

        <div className="setting-row">
          <label htmlFor="asr-threads">线程数</label>
          <input
            id="asr-threads"
            aria-label="Thread count"
            type="number"
            min={1}
            max={16}
            value={settings.numThreads}
            onChange={(e) => {
              const value = Number.parseInt(e.target.value, 10)
              if (value >= 1 && value <= 16) {
                onSave({ ...settings, numThreads: value })
              }
            }}
            className="asr-settings-form__stepper"
          />
        </div>

        <div className="setting-row">
          <label className="asr-settings-form__toggle-row">
            <input
              type="checkbox"
              aria-label="VAD"
              checked={settings.vadEnabled}
              onChange={(e) => onSave({ ...settings, vadEnabled: e.target.checked })}
            />
            <span>启用 VAD 语音活动检测</span>
          </label>
        </div>

        <div className="setting-row">
          <label className="asr-settings-form__toggle-row">
            <input
              type="checkbox"
              aria-label="Auto-transcribe"
              checked={settings.autoTranscribeImports}
              onChange={(e) => onSave({ ...settings, autoTranscribeImports: e.target.checked })}
            />
            <span>导入音频后自动转写</span>
          </label>
        </div>
      </section>

      <section className="settings-section">
        <h2>{isSenseVoice ? 'SenseVoice 选项' : 'Whisper 选项'}</h2>

        {isSenseVoice && settings.providerOptions.kind === 'sense_voice' && (
          <div className="setting-row">
            <label className="asr-settings-form__toggle-row">
              <input
                type="checkbox"
                aria-label="ITN"
                checked={settings.providerOptions.useItn}
                onChange={(e) =>
                  onSave({
                    ...settings,
                    providerOptions: { kind: 'sense_voice', useItn: e.target.checked },
                  })
                }
              />
              <span>启用 ITN（逆文本标准化）</span>
            </label>
          </div>
        )}

        {!isSenseVoice && settings.providerOptions.kind === 'whisper' && (
          <div className="provider-selector" role="radiogroup" aria-label="Whisper 任务">
            <button
              className={`provider-selector__option ${settings.providerOptions.task === 'transcribe' ? 'provider-selector__option--active' : ''}`}
              role="radio"
              aria-checked={settings.providerOptions.task === 'transcribe'}
              aria-label="Transcribe"
              onClick={() =>
                onSave({
                  ...settings,
                  providerOptions: { kind: 'whisper', task: 'transcribe' as WhisperTask },
                })
              }
            >
              <span>Transcribe</span>
            </button>
            <button
              className={`provider-selector__option ${settings.providerOptions.task === 'translate' ? 'provider-selector__option--active' : ''}`}
              role="radio"
              aria-checked={settings.providerOptions.task === 'translate'}
              aria-label="Translate"
              onClick={() =>
                onSave({
                  ...settings,
                  providerOptions: { kind: 'whisper', task: 'translate' as WhisperTask },
                })
              }
            >
              <span>Translate</span>
            </button>
          </div>
        )}
      </section>

      <div className="asr-settings-form__footer">
        <button
          className="button button--primary"
          aria-label="Save settings"
          onClick={handleSave}
          disabled={saving}
        >
          <Save size={14} />
          {saving ? '保存中...' : '保存设置'}
        </button>
      </div>
    </div>
  )
}