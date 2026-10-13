import { AudioLines, Copy, Download, ExternalLink, RefreshCw } from 'lucide-react'
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import {
  exportDiagnostics,
  loadDiagnostics,
  openMacosPrivacySettings,
  probeSelectedMicrophone,
} from '../data/adapter'
import type { CoreDiagnosticsSnapshot } from '../services/lifesub'

export function DiagnosticsSettings() {
  const [snapshot, setSnapshot] = useState<CoreDiagnosticsSnapshot | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [probing, setProbing] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const generation = useRef(0)

  const refresh = useCallback(async (initial = false) => {
    const current = ++generation.current
    if (initial) setLoading(true)
    else setRefreshing(true)
    setError(null)
    setMessage(null)
    try {
      const value = await loadDiagnostics()
      if (current !== generation.current) return
      setSnapshot(value)
    } catch (cause) {
      if (current !== generation.current) return
      setSnapshot(null)
      setError(errorText(cause, '无法加载诊断'))
    } finally {
      if (current === generation.current) {
        setLoading(false)
        setRefreshing(false)
      }
    }
  }, [])

  useEffect(() => {
    void refresh(true)
  }, [refresh])

  const exportSnapshot = async () => {
    if (!snapshot) return
    setError(null)
    setMessage(null)
    try {
      const path = await exportDiagnostics(snapshot)
      if (path) setMessage('诊断已导出')
    } catch (cause) {
      setError(errorText(cause, '导出诊断失败'))
    }
  }

  const openPrivacy = async (target: 'microphone' | 'screen_recording') => {
    setError(null)
    try {
      await openMacosPrivacySettings(target)
    } catch (cause) {
      setError(errorText(cause, '无法打开系统设置'))
    }
  }

  const retestSelectedMicrophone = async () => {
    const uid = snapshot?.selected_microphone.uid
    if (!uid || !snapshot?.selected_microphone.connected || probing) return
    setProbing(true)
    setError(null)
    setMessage(null)
    try {
      await probeSelectedMicrophone(uid)
      await refresh()
      setMessage('麦克风测试已完成，诊断已刷新')
    } catch (cause) {
      setError(errorText(cause, '麦克风测试失败'))
    } finally {
      setProbing(false)
    }
  }

  const copyFact = async (label: string, value: string) => {
    setError(null)
    setMessage(null)
    try {
      await navigator.clipboard.writeText(value)
      setMessage(`已复制${label}`)
    } catch (cause) {
      setError(errorText(cause, `无法复制${label}`))
    }
  }

  return (
    <div className="settings-tab-content diagnostics-settings">
      <div className="diagnostics-settings__heading">
        <h1>诊断</h1>
        <div className="diagnostics-settings__tools">
          <button type="button" className="icon-button" aria-label="刷新诊断" title="刷新诊断" disabled={loading} onClick={() => void refresh()}>
            <RefreshCw size={16} className={refreshing ? 'microphone-selector__refresh-icon--active' : ''} aria-hidden="true" />
          </button>
          <button type="button" className="icon-button" aria-label="导出诊断" title="导出诊断" disabled={!snapshot || loading || refreshing} onClick={() => void exportSnapshot()}>
            <Download size={16} aria-hidden="true" />
          </button>
        </div>
      </div>

      {loading && <p className="settings-feedback" role="status">正在读取诊断</p>}
      {error && (
        <div className="settings-inline-status">
          <p className="settings-feedback settings-feedback--error" role="status">{error}</p>
          <button type="button" className="button" onClick={() => void refresh()}>重试诊断</button>
        </div>
      )}
      {message && <p className="settings-feedback" role="status">{message}</p>}

      {snapshot && (
        <>
          <div className="diagnostics-summary" role="status">
            <strong>{diagnosticReviewCount(snapshot) === 0 ? '当前快照未见需核对项' : `${diagnosticReviewCount(snapshot)} 项状态需核对`}</strong>
            <span>权限与设备是否必需取决于录制模式；请结合下方事实判断。</span>
          </div>
          <section className="settings-section">
            <h2>权限与设备</h2>
            <DiagnosticRow label="麦克风权限" value={permissionLabel(snapshot.permissions.microphone)} tone={permissionTone(snapshot.permissions.microphone)} action={<PrivacyButton target="microphone" onOpen={openPrivacy} />} />
            <DiagnosticRow label="录屏与系统音频" value={permissionLabel(snapshot.permissions.screen_recording)} tone={permissionTone(snapshot.permissions.screen_recording)} action={<PrivacyButton target="screen_recording" onOpen={openPrivacy} />} />
            <DiagnosticRow label="选中麦克风" value={snapshot.selected_microphone.name ?? '未选择'} detail={snapshot.selected_microphone.uid ?? undefined} copyLabel={snapshot.selected_microphone.uid ? '麦克风 UID' : undefined} copyValue={snapshot.selected_microphone.uid ?? undefined} onCopy={copyFact} />
            {snapshot.selected_microphone.transport && <DiagnosticRow label="设备类型" value={snapshot.selected_microphone.transport} copyLabel="麦克风设备类型" copyValue={snapshot.selected_microphone.transport} onCopy={copyFact} />}
            <DiagnosticRow label="设备状态" value={deviceStatus(snapshot)} tone={!snapshot.selected_microphone.uid ? 'muted' : snapshot.selected_microphone.connected ? 'success' : 'error'} />
            <DiagnosticRow label="首帧测试" value={probeLabel(snapshot.selected_microphone.probe_result)} tone={snapshot.selected_microphone.probe_result === 'compatible' ? 'success' : 'muted'} action={snapshot.selected_microphone.uid && snapshot.selected_microphone.connected ? (
              <button type="button" className="icon-button" aria-label="重新测试选中麦克风" title="重新测试选中麦克风" disabled={probing || refreshing} onClick={() => void retestSelectedMicrophone()}>
                <AudioLines size={15} aria-hidden="true" />
              </button>
            ) : undefined} />
          </section>

          <section className="settings-section">
            <h2>运行时身份</h2>
            <DiagnosticRow label="应用版本" value={snapshot.identities.app_version} detail={snapshot.identities.app_executable_sha256} copyLabel="应用可执行文件 SHA-256" copyValue={snapshot.identities.app_executable_sha256} onCopy={copyFact} />
            <DiagnosticRow label="Capture Helper" value={`协议 V${snapshot.identities.capture_protocol_version}`} detail={snapshot.identities.helper_code_sha256} copyLabel="Capture Helper SHA-256" copyValue={snapshot.identities.helper_code_sha256} onCopy={copyFact} />
            <DiagnosticRow label="ASR 运行时" value={`sherpa-onnx ${snapshot.identities.runtime_version}`} detail={snapshot.identities.runtime_archive_sha256} copyLabel="ASR 运行时 SHA-256" copyValue={snapshot.identities.runtime_archive_sha256} onCopy={copyFact} />
            <DiagnosticRow label="Runtime build" value={snapshot.identities.runtime_build_id} detail={snapshot.identities.runtime_git_commit} copyLabel="Runtime Git commit" copyValue={snapshot.identities.runtime_git_commit} onCopy={copyFact} />
          </section>

          <section className="settings-section">
            <h2>模型</h2>
            {snapshot.models.map((model) => (
              <div key={model.model_id} className="diagnostic-model">
                <DiagnosticRow label={model.model_id} value={model.state} tone={model.state === 'runtime_qualified' ? 'success' : 'muted'} />
                <DiagnosticRow label="Bundle identity" value={model.bundle_identity} copyLabel={`${model.model_id} bundle identity`} copyValue={model.bundle_identity} onCopy={copyFact} />
                <DiagnosticRow label="Runtime identity SHA-256" value={model.runtime_identity_sha256 ?? '未记录'} copyLabel={model.runtime_identity_sha256 ? `${model.model_id} runtime identity SHA-256` : undefined} copyValue={model.runtime_identity_sha256 ?? undefined} onCopy={copyFact} />
                {model.last_error_code && <DiagnosticRow label="模型错误" value={model.last_error_code} tone="error" copyLabel={`${model.model_id} 错误代码`} copyValue={model.last_error_code} onCopy={copyFact} />}
              </div>
            ))}
            {snapshot.models.length === 0 && <p className="empty-state">没有已记录的模型安装。</p>}
          </section>

          <section className="settings-section">
            <h2>存储</h2>
            <DiagnosticRow label="数据目录" value={snapshot.storage.data_directory} copyLabel="数据目录" copyValue={snapshot.storage.data_directory} onCopy={copyFact} />
            <DiagnosticRow label="Catalog" value={formatBytes(snapshot.storage.catalog_bytes)} />
            <DiagnosticRow label="原始音频" value={formatBytes(snapshot.storage.audio_bytes)} />
            <DiagnosticRow label="模型" value={formatBytes(snapshot.storage.model_bytes)} />
          </section>

          <section className="settings-section">
            <h2>最近错误</h2>
            {snapshot.recent_error ? (
              <DiagnosticRow label={snapshot.recent_error.code} value={snapshot.recent_error.summary} tone="error" copyLabel="错误代码" copyValue={snapshot.recent_error.code} onCopy={copyFact} />
            ) : <p className="empty-state">没有已记录的 capture 错误。</p>}
          </section>
        </>
      )}
    </div>
  )
}

function PrivacyButton({ target, onOpen }: { target: 'microphone' | 'screen_recording'; onOpen: (target: 'microphone' | 'screen_recording') => Promise<void> }) {
  const label = target === 'microphone' ? '打开麦克风隐私设置' : '打开录屏隐私设置'
  return (
    <button type="button" className="icon-button" aria-label={label} title={label} onClick={() => void onOpen(target)}>
      <ExternalLink size={15} aria-hidden="true" />
    </button>
  )
}

function DiagnosticRow({ label, value, detail, tone = 'muted', action, copyLabel, copyValue, onCopy }: { label: string; value: string; detail?: string; tone?: 'success' | 'error' | 'muted'; action?: ReactNode; copyLabel?: string; copyValue?: string; onCopy?: (label: string, value: string) => Promise<void> }) {
  return (
    <div className="diagnostic-row">
      <span className="diagnostic-row__label">{label}</span>
      <div className="diagnostic-row__value">
        <span className={`status-pill ${tone === 'success' ? '' : tone === 'error' ? 'status-pill--error' : 'status-pill--quiet'}`}>{value}</span>
        {detail && <small title={detail}>{detail}</small>}
      </div>
      {(action || (copyLabel && copyValue && onCopy)) && (
        <div className="diagnostic-row__action">
          {action}
          {copyLabel && copyValue && onCopy && (
            <button type="button" className="icon-button" aria-label={`复制${copyLabel}`} title={`复制${copyLabel}`} onClick={() => void onCopy(copyLabel, copyValue)}>
              <Copy size={15} aria-hidden="true" />
            </button>
          )}
        </div>
      )}
    </div>
  )
}

function permissionLabel(value: string) {
  return value === 'granted' ? '已授权' : value === 'not_granted' ? '未授权' : value === 'not_determined' ? '尚未请求' : value === 'restricted' ? '受系统限制' : '已拒绝'
}

function permissionTone(value: string): 'success' | 'error' | 'muted' {
  return value === 'granted' ? 'success' : value === 'not_determined' ? 'muted' : 'error'
}

function deviceStatus(snapshot: CoreDiagnosticsSnapshot) {
  if (!snapshot.selected_microphone.uid) return '未选择'
  return snapshot.selected_microphone.connected ? '已连接' : '已断开'
}

function probeLabel(value: string | null) {
  if (value === 'compatible') return '已通过首帧测试'
  if (!value) return '未测试'
  return value.replaceAll('_', ' ')
}

function diagnosticReviewCount(snapshot: CoreDiagnosticsSnapshot) {
  let count = 0
  if (snapshot.permissions.microphone !== 'granted') count += 1
  if (snapshot.permissions.screen_recording !== 'granted') count += 1
  if (!snapshot.selected_microphone.uid || !snapshot.selected_microphone.connected) count += 1
  if (snapshot.selected_microphone.probe_result && snapshot.selected_microphone.probe_result !== 'compatible') count += 1
  if (snapshot.recent_error) count += 1
  return count
}

function formatBytes(value: number) {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`
  if (value >= 1024 * 1024) return `${Math.round(value / (1024 * 1024))} MB`
  if (value >= 1024) return `${Math.round(value / 1024)} KB`
  return `${value} B`
}

function errorText(error: unknown, fallback: string) {
  return error instanceof Error && error.message ? error.message : fallback
}
