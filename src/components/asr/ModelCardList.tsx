import { AlertCircle, CheckCircle2, Download, Trash2, XCircle } from 'lucide-react'
import type { ModelInfo } from '../../domain'

const MB = 1_000_000

function formatBytes(bytes: number): string {
  if (bytes >= MB) return `${(bytes / MB).toFixed(1)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

interface ModelCardListProps {
  models: ModelInfo[]
  selectedModelId: string
  onSelect: (modelId: string) => void
  onDownload: (modelId: string) => void
  onCancel: (downloadId: string) => void
  onDelete: (modelId: string) => void
}

export function ModelCardList({
  models,
  selectedModelId,
  onSelect,
  onDownload,
  onCancel,
  onDelete,
}: ModelCardListProps) {
  if (models.length === 0) {
    return (
      <p className="model-card-list__empty">
        当前 Provider 没有可用模型。
      </p>
    )
  }

  return (
    <div className="model-card-list" role="listbox" aria-label="选择模型">
      {models.map((model) => {
        const isSelected = model.modelId === selectedModelId
        const download = model.downloadState
        const isDownloading = download?.state === 'downloading' || download?.state === 'verifying' || download?.state === 'installing'
        const isFailed = download?.state === 'failed'

        return (
          <div
            key={model.modelId}
            className={`model-card ${isSelected ? 'model-card--selected' : ''}`}
            role="option"
            aria-selected={isSelected}
            onClick={() => onSelect(model.modelId)}
          >
            <div className="model-card__info">
              <div className="model-card__header">
                <strong>{model.displayName}</strong>
                {model.recommended && <span className="model-card__badge">推荐</span>}
                {model.installed && <CheckCircle2 size={14} className="model-card__installed-icon" />}
              </div>
              <p className="model-card__description">{model.description}</p>
              <div className="model-card__meta">
                <span>{formatBytes(model.sizeBytes)}</span>
                <span>{model.license}</span>
                <span>{model.languages.join(', ')}</span>
              </div>
            </div>

            {isDownloading && download && (
              <div className="model-card__progress">
                <div className="model-card__progress-bar">
                  <div
                    className="model-card__progress-fill"
                    style={{ width: `${(download.downloadedBytes / download.expectedBytes) * 100}%` }}
                  />
                </div>
                <small>
                  {formatBytes(download.downloadedBytes)} / {formatBytes(download.expectedBytes)}
                </small>
                <button
                  className="icon-button"
                  aria-label="Cancel download"
                  onClick={(e) => {
                    e.stopPropagation()
                    onCancel(download.id)
                  }}
                >
                  <XCircle size={14} />
                </button>
              </div>
            )}

            {isFailed && download && (
              <div className="model-card__error">
                <AlertCircle size={14} />
                <small>{download.errorCode ?? '下载失败'}</small>
                <button
                  className="text-button"
                  onClick={(e) => {
                    e.stopPropagation()
                    onDownload(model.modelId)
                  }}
                >
                  重试
                </button>
              </div>
            )}

            <div className="model-card__actions">
              {!model.installed && !isDownloading && !isFailed && (
                <button
                  className="button button--primary"
                  aria-label="Download model"
                  onClick={(e) => {
                    e.stopPropagation()
                    onDownload(model.modelId)
                  }}
                >
                  <Download size={14} />
                  下载
                </button>
              )}
              {model.installed && (
                <button
                  className="button button--danger"
                  aria-label="Delete model"
                  onClick={(e) => {
                    e.stopPropagation()
                    onDelete(model.modelId)
                  }}
                >
                  <Trash2 size={14} />
                  删除
                </button>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}