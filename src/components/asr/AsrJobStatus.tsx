import { AlertCircle, CheckCircle2, Loader, RotateCcw, XCircle } from 'lucide-react'
import type { AsrJobSummary } from '../../services/lifesub'

const STATE_LABELS: Record<string, string> = {
  queued: '排队中',
  blocked_model: '等待模型',
  preparing: '准备中',
  transcribing: '转写中',
  succeeded: '已完成',
  failed: '失败',
  cancelled: '已取消',
}

const STATE_ICONS: Record<string, typeof AlertCircle> = {
  queued: Loader,
  blocked_model: AlertCircle,
  preparing: Loader,
  transcribing: Loader,
  succeeded: CheckCircle2,
  failed: XCircle,
  cancelled: XCircle,
}

interface AsrJobStatusProps {
  job: AsrJobSummary
  onCancel: (jobId: string) => void
  onRetry: (jobId: string) => void
}

export function AsrJobStatus({ job, onCancel, onRetry }: AsrJobStatusProps) {
  const isTerminal = job.state === 'succeeded' || job.state === 'failed' || job.state === 'cancelled'
  const isActive = !isTerminal
  const Icon = STATE_ICONS[job.state] ?? AlertCircle
  const label = STATE_LABELS[job.state] ?? job.state

  return (
    <div className={`asr-job asr-job--${job.state}`} role="status" aria-label={`ASR 任务: ${label}`}>
      <div className="asr-job__header">
        <Icon size={16} className={`asr-job__icon asr-job__icon--${job.state}`} />
        <span className="asr-job__label">{label}</span>
        <span className="asr-job__provider">{job.provider} · {job.modelId}</span>
      </div>

      {job.errorCode && (
        <div className="asr-job__error" role="alert">
          <AlertCircle size={14} />
          <span>{job.errorSummary ?? job.errorCode}</span>
        </div>
      )}

      <div className="asr-job__actions">
        {isActive && (
          <button
            className="text-button text-button--danger"
            onClick={() => onCancel(job.id)}
            aria-label="取消转写"
          >
            <XCircle size={14} />取消
          </button>
        )}
        {job.state === 'failed' && (
          <button
            className="text-button"
            onClick={() => onRetry(job.id)}
            aria-label="重试转写"
          >
            <RotateCcw size={14} />重试
          </button>
        )}
        {job.state === 'cancelled' && (
          <button
            className="text-button"
            onClick={() => onRetry(job.id)}
            aria-label="重新开始"
          >
            <RotateCcw size={14} />重新开始
          </button>
        )}
      </div>
    </div>
  )
}