import { Loader2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import type { AsrSettings, ModelInfo } from '../domain'
import {
  cancelModelDownload,
  deleteAsrModel,
  downloadAsrModel,
  getAsrSettings,
  isTauriRuntime,
  listAsrModels,
  saveAsrSettings,
} from '../services/asr'
import { AsrSettingsForm } from './asr/AsrSettingsForm'

const BROWSER_MODEL_CATALOG: ModelInfo[] = [
  {
    modelId: 'sense-voice-small-int8-2024-07-17',
    provider: 'sense_voice',
    displayName: 'SenseVoice Small INT8',
    description: '默认中文/中英混合模型，支持 ITN 逆文本标准化。',
    sizeBytes: 163_002_883,
    license: 'Apache-2.0',
    languages: ['zh', 'en', 'ja', 'ko', 'yue'],
    recommended: true,
    installed: false,
    downloadState: null,
  },
  {
    modelId: 'whisper-tiny',
    provider: 'whisper',
    displayName: 'Whisper Tiny',
    description: '最小模型，适合快速验证和低资源环境。',
    sizeBytes: 116_204_861,
    license: 'MIT',
    languages: ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es'],
    recommended: false,
    installed: false,
    downloadState: null,
  },
  {
    modelId: 'whisper-base',
    provider: 'whisper',
    displayName: 'Whisper Base',
    description: '均衡精度与速度，推荐作为默认 Whisper 模型。',
    sizeBytes: 207_557_382,
    license: 'MIT',
    languages: ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es'],
    recommended: true,
    installed: false,
    downloadState: null,
  },
  {
    modelId: 'whisper-small',
    provider: 'whisper',
    displayName: 'Whisper Small',
    description: '更高精度，适合对质量要求更高的场景。',
    sizeBytes: 639_387_718,
    license: 'MIT',
    languages: ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es'],
    recommended: false,
    installed: false,
    downloadState: null,
  },
]

const DEFAULT_SETTINGS: AsrSettings = {
  provider: 'sense_voice',
  modelId: 'sense-voice-small-int8-2024-07-17',
  language: 'zh',
  numThreads: 4,
  vadEnabled: true,
  autoTranscribeImports: false,
  providerOptions: { kind: 'sense_voice', useItn: true },
}

export function SettingsView() {
  const [settings, setSettings] = useState<AsrSettings | null>(null)
  const [models, setModels] = useState<ModelInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const tauri = isTauriRuntime()

  useEffect(() => {
    if (!tauri) {
      setSettings(DEFAULT_SETTINGS)
      setModels(BROWSER_MODEL_CATALOG)
      setLoading(false)
      return
    }

    let cancelled = false
    async function load() {
      try {
        const [s, m] = await Promise.all([getAsrSettings(), listAsrModels()])
        if (!cancelled) {
          setSettings(s)
          setModels(m)
        }
      } catch (err) {
        if (!cancelled) {
          setError(`加载 ASR 设置失败：${String(err)}`)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    load()
    return () => {
      cancelled = true
    }
  }, [tauri])

  const handleSave = useCallback(
    async (updated: AsrSettings) => {
      setError(null)
      if (!tauri) {
        setSettings(updated)
        return
      }
      try {
        const saved = await saveAsrSettings(updated)
        setSettings(saved)
      } catch (err) {
        setError(`保存设置失败：${String(err)}`)
      }
    },
    [tauri],
  )

  const handleDownloadModel = useCallback(
    async (modelId: string) => {
      if (!tauri) return
      setError(null)
      try {
        await downloadAsrModel(modelId)
        const refreshed = await listAsrModels()
        setModels(refreshed)
      } catch (err) {
        setError(`模型下载失败：${String(err)}`)
      }
    },
    [tauri],
  )

  const handleCancelDownload = useCallback(
    async (downloadId: string) => {
      if (!tauri) return
      setError(null)
      try {
        await cancelModelDownload(downloadId)
        const refreshed = await listAsrModels()
        setModels(refreshed)
      } catch (err) {
        setError(`取消下载失败：${String(err)}`)
      }
    },
    [tauri],
  )

  const handleDeleteModel = useCallback(
    async (modelId: string) => {
      if (!tauri) return
      setError(null)
      try {
        await deleteAsrModel(modelId)
        const refreshed = await listAsrModels()
        setModels(refreshed)
      } catch (err) {
        setError(`删除模型失败：${String(err)}`)
      }
    },
    [tauri],
  )

  return (
    <main className="settings-view">
      <header>
        <span className="eyebrow">Local ASR</span>
        <h1>设置</h1>
        <p>
          {tauri
            ? '选择本地 ASR Provider 与模型。所有处理在本机完成，音频不会上传。'
            : '浏览器预览模式：以下为可用模型目录，下载与转写功能仅在桌面版中可用。'}
        </p>
      </header>

      {!tauri && (
        <div className="notice" role="status">
          <Loader2 size={14} />
          <span>浏览器预览模式 — 模型目录仅供参考，无法执行下载或转写。</span>
        </div>
      )}

      <AsrSettingsForm
        settings={settings}
        models={models}
        onSave={handleSave}
        onDownloadModel={handleDownloadModel}
        onCancelDownload={handleCancelDownload}
        onDeleteModel={handleDeleteModel}
        loading={loading}
        error={error}
      />
    </main>
  )
}