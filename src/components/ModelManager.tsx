import { Download, RefreshCw, RotateCw, X } from 'lucide-react'
import { useMemo } from 'react'
import type { CoreAsrModel } from '../services/lifesub'
import { useModelManagerController } from '../modelManager'

export function ModelManager() {
  const manager = useModelManagerController()
  const { models, loading, refreshing, error, pendingModelId, cancelRequestedDownloadId } = manager

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
      <div className="diagnostics-settings__heading">
        <h1>模型</h1>
        <button type="button" className="icon-button" aria-label="刷新模型状态" title="刷新模型状态" disabled={loading} onClick={() => void manager.refresh()}>
          <RefreshCw size={16} className={refreshing ? 'microphone-selector__refresh-icon--active' : ''} aria-hidden="true" />
        </button>
      </div>

      {error && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{error}</p>
          <button type="button" className="button" onClick={() => void manager.refresh()}>
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
              {providerModels.map((model) => {
                const presentation = modelPresentation(model)
                return <article key={`${model.model_id}:${model.manifest_version}:${model.bundle_identity}`} className="model-card">
                  <div className="model-card__main">
                    <strong>{model.display_name}</strong>
                    <small>
                      {formatRuntime(model.runtime_family, model.runtime_version)} · {formatBytes(model.total_bytes)}
                    </small>
                    <small>{model.license_spdx} · {formatLanguages(model.supported_languages)}</small>
                    {model.download && <ModelProgress model={model} />}
                    <ModelStageTimeline model={model} />
                    {presentation.summary && <p className="model-card__error-summary">{presentation.summary}</p>}
                    {presentation.technicalDetail && (
                      <details className="model-card__technical-details">
                        <summary>技术详情</summary>
                        <code>{presentation.technicalDetail}</code>
                      </details>
                    )}
                  </div>
                  <div className="model-card__meta">
                    <span className={`status-pill ${presentation.tone === 'success' ? '' : presentation.tone === 'error' ? 'status-pill--error' : 'status-pill--quiet'}`}>
                      {presentation.label}
                    </span>
                    {model.download && isRetryable(model.download.state) ? (
                      <button type="button" className="button" aria-label={`重试 ${model.display_name} 下载`} disabled={pendingModelId !== null} onClick={() => void manager.retry(model.model_id, model.download!.id)}>
                        <RotateCw size={15} aria-hidden="true" />重试
                      </button>
                    ) : model.download && isActive(model.download.state) ? (
                      <button type="button" className="button" aria-label={`取消 ${model.display_name} 下载`} disabled={cancelRequestedDownloadId === model.download.id} onClick={() => void manager.cancel(model.model_id, model.download!.id)}>
                        <X size={15} aria-hidden="true" />{cancelRequestedDownloadId === model.download.id ? '正在取消' : '取消'}
                      </button>
                    ) : model.installation_state === 'runtime_qualified' ? null : model.installable ? (
                      <button
                        type="button"
                        className="button"
                        aria-label={`安装 ${model.display_name}`}
                        disabled={pendingModelId !== null}
                        onClick={() => void manager.install(model.model_id)}
                      >
                        <Download size={15} aria-hidden="true" />安装
                      </button>
                    ) : null}
                  </div>
                </article>
              })}
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

const MODEL_STAGES = [
  { state: 'downloading', label: '下载文件' },
  { state: 'verifying', label: '校验文件' },
  { state: 'installing', label: '安装文件' },
  { state: 'qualifying', label: '运行时验证' },
] as const

function ModelStageTimeline({ model }: { model: CoreAsrModel }) {
  const downloadState = model.download?.state
  const activeStage = MODEL_STAGES.findIndex((stage) => stage.state === downloadState)
  const queued = downloadState === 'queued'
  const qualified = model.installation_state === 'runtime_qualified'
  const installed = model.installation_state === 'installed_unqualified'
  if (!qualified && !installed && !queued && activeStage < 0) return null

  return (
    <ol className="model-stage-timeline" aria-label={`${model.display_name} 安装步骤`}>
      {MODEL_STAGES.map((stage, index) => {
        const progress = qualified || (installed && index < MODEL_STAGES.length - 1) || activeStage > index
          ? 'done'
          : (queued && index === 0) || activeStage === index
            ? 'current'
            : 'pending'
        return <li key={stage.state} data-stage={progress}>{stage.label}</li>
      })}
    </ol>
  )
}

function modelPresentation(model: CoreAsrModel): { label: string; tone: 'success' | 'error' | 'muted'; summary?: string; technicalDetail?: string } {
  const download = model.download
  if (download && isActive(download.state)) return { label: stageLabel(download.state), tone: 'muted' }
  if (download?.state === 'failed') {
    return {
      label: '下载失败',
      tone: 'error',
      summary: download.error_summary ?? '模型下载未完成，可以重试。',
      technicalDetail: download.error_code ?? undefined,
    }
  }
  if (download?.state === 'cancelled') return { label: '下载已取消', tone: 'muted' }
  if (model.last_error_code) {
    return {
      label: model.installation_state === 'installed_unqualified' ? '运行时验证失败' : '模型不可用',
      tone: 'error',
      summary: model.installation_state === 'installed_unqualified'
        ? '模型文件已安装，但尚未通过当前运行时验证。'
        : '模型当前不可用，请查看技术详情。',
      technicalDetail: model.last_error_code,
    }
  }
  if (model.reason_code === 'desktop_runtime_required') {
    return {
      label: '暂不可安装',
      tone: 'muted',
      summary: '请在 LifeSub 桌面应用中管理和安装本地模型。',
    }
  }
  switch (model.installation_state) {
    case 'runtime_qualified':
      return { label: '已就绪', tone: 'success' }
    case 'installed_unqualified':
      return { label: '等待运行时验证', tone: 'muted' }
    default:
      return { label: model.installable ? '未安装' : '暂不可安装', tone: 'muted' }
  }
}

function ModelProgress({ model }: { model: CoreAsrModel }) {
  const download = model.download!
  const percentage = download.expected_bytes > 0
    ? Math.min(100, Math.round(download.downloaded_bytes * 100 / download.expected_bytes))
    : 0
  return (
    <div className="model-progress">
      <div className="model-progress__labels">
        <span>{stageLabel(download.state)} · 文件 {download.artifact_index}/{download.artifact_count}</span>
        <span>{formatBytes(download.downloaded_bytes)} / {formatBytes(download.expected_bytes)}</span>
      </div>
      <progress aria-label={`${model.display_name} 下载进度`} value={percentage} max={100} aria-valuenow={percentage} />
    </div>
  )
}

function stageLabel(state: string) {
  switch (state) {
    case 'queued': return '等待下载'
    case 'downloading': return '下载中'
    case 'verifying': return '校验中'
    case 'installing': return '安装中'
    case 'qualifying': return '运行时验证中'
    case 'succeeded': return '下载完成'
    case 'cancelled': return '已取消'
    case 'failed': return '下载失败'
    default: return state
  }
}

function isActive(state: string) {
  return ['queued', 'downloading', 'verifying', 'installing', 'qualifying'].includes(state)
}

function isRetryable(state: string) { return state === 'failed' || state === 'cancelled' }

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
  if (bytes >= 1024 * 1024) return `${Math.round(bytes / (1024 * 1024))} MB`
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`
  return `${bytes} B`
}
