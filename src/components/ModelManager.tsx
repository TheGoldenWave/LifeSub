import { useEffect, useMemo, useState } from 'react'
import { loadModelCatalog } from '../data/adapter'
import type { CoreAsrModel } from '../services/lifesub'

export function ModelManager() {
  const [models, setModels] = useState<CoreAsrModel[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void reload()
  }, [])

  const reload = async () => {
    setLoading(true)
    setError(null)
    try {
      setModels(await loadModelCatalog())
    } catch (cause) {
      setModels([])
      setError(errorText(cause))
    } finally {
      setLoading(false)
    }
  }

  const groupedModels = useMemo(() => {
    const groups = new Map<string, CoreAsrModel[]>()
    for (const model of models) {
      const key = providerLabel(model.provider)
      const list = groups.get(key) ?? []
      list.push(model)
      groups.set(key, list)
    }
    return groups
  }, [models])

  return (
    <div className="settings-tab-content">
      <span className="eyebrow">MODELS</span>
      <h1>模型</h1>

      {error && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{error}</p>
          <button type="button" className="button" onClick={() => void reload()}>
            重试加载模型
          </button>
        </div>
      )}

      <section className="settings-section">
        <h2>当前清单</h2>
        <div className="model-list">
          {Array.from(groupedModels.entries()).map(([provider, providerModels]) => (
            <div key={provider}>
              <div className="model-list__provider">{provider}</div>
              {providerModels.map((model) => (
                <article key={`${model.model_id}:${model.manifest_version}:${model.bundle_identity}`} className="model-card">
                  <div className="model-card__main">
                    <strong>{model.display_name}</strong>
                    <small>
                      {formatRuntime(model.runtime_family, model.runtime_version)} · {formatBytes(model.total_bytes)}
                    </small>
                    <small>{model.license_spdx} · {formatLanguages(model.supported_languages)}</small>
                  </div>
                  <div className="model-card__meta">
                    <span className={`status-pill ${model.installation_state === 'runtime_qualified' ? '' : 'status-pill--quiet'}`}>
                      {installationLabel(model.installation_state, model.last_error_code)}
                    </span>
                    {model.installation_state === 'runtime_qualified' ? (
                      <button type="button" className="button" disabled aria-disabled="true">
                        管理计划中
                      </button>
                    ) : (
                      <span className="status-pill status-pill--quiet">
                        暂不可安装
                      </span>
                    )}
                  </div>
                </article>
              ))}
            </div>
          ))}
          {!loading && !error && models.length === 0 && (
            <p className="empty-state">当前没有可展示的模型清单。</p>
          )}
        </div>
      </section>
    </div>
  )
}

function installationLabel(state: string, lastErrorCode: string | null) {
  if (lastErrorCode) return `错误：${lastErrorCode}`
  switch (state) {
    case 'runtime_qualified':
      return '已就绪'
    case 'installed_unqualified':
      return '等待运行时验证'
    default:
      return '未安装'
  }
}

function providerLabel(provider: string) {
  switch (provider) {
    case 'sense_voice':
      return 'SenseVoice（推荐）'
    case 'whisper':
      return 'Whisper'
    case 'qwen3_asr':
      return 'Qwen3-ASR'
    default:
      return provider
  }
}

function formatLanguages(languages: string[]) {
  return languages.join(' / ')
}

function formatRuntime(runtimeFamily: string, version: string) {
  return `${runtimeFamily.replaceAll('_', '-')} ${version}`
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
  }
  return `${Math.round(bytes / (1024 * 1024))} MB`
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : '加载失败'
}
