import { Archive, AudioLines, Import, Settings } from 'lucide-react'
import { useMemo, useState } from 'react'
import { RecordList } from './components/RecordList'
import { RecorderBar } from './components/RecorderBar'
import { TranscriptView } from './components/TranscriptView'
import { SettingsView } from './components/SettingsView'
import { demoRecords } from './data/demo'
import type { CaptureState, EvidenceRecord, TranscriptRevision } from './domain'
import { open } from '@tauri-apps/plugin-dialog'
import { appendTranscriptRevision, createCapture, importAudio, isTauriRuntime, transitionCapture, type CoreCaptureSession } from './services/lifesub'

export default function App() {
  const [captureState, setCaptureState] = useState<CaptureState>('idle')
  const [records, setRecords] = useState<EvidenceRecord[]>(demoRecords)
  const [selectedId, setSelectedId] = useState(records[0].id)
  const [query, setQuery] = useState('')
  const [coreSession, setCoreSession] = useState<CoreCaptureSession | null>(null)
  const [notice, setNotice] = useState('')
  const [view, setView] = useState<'timeline' | 'settings'>('timeline')
  const selectedRecord = useMemo(() => records.find((record) => record.id === selectedId) ?? records[0], [records, selectedId])

  const updateRevision = async (revision: TranscriptRevision) => {
    setRecords((current) => current.map((record) => record.id === selectedId ? { ...record, revision } : record))
    if (!isTauriRuntime() || !selectedId.startsWith('rec_')) return
    try {
      await appendTranscriptRevision(selectedId, 'manual', revision.segments.map((segment) => ({
        id: `seg_${crypto.randomUUID().replaceAll('-', '')}`,
        start_ms: segment.startMs,
        end_ms: segment.endMs,
        source: segment.source === '系统音频' ? 'system_audio' : segment.source === '导入音频' ? 'imported' : 'microphone',
        text: segment.text,
      })))
    } catch (error) {
      setNotice(`修订已保留在当前界面，但未能写入本地 Catalog：${String(error)}`)
    }
  }

  const changeCaptureState = async (target: CaptureState) => {
    const previousState = captureState
    setCaptureState(target)
    if (!isTauriRuntime()) return
    try {
      let session = coreSession
      if ((!session || session.state === 'stopped') && target === 'recording') {
        session = await createCapture(`记录 ${new Date().toLocaleString('zh-CN')}`)
      }
      if (session) {
        const transitioned = await transitionCapture(session, target)
        setCoreSession(transitioned)
      }
    } catch (error) {
      setCaptureState(previousState)
      setNotice(`录音状态未能保存：${String(error)}`)
    }
  }

  const handleImport = async () => {
    if (!isTauriRuntime()) {
      setNotice('浏览器预览使用演示数据；在桌面版中可选择本机音频文件。')
      return
    }
    try {
      const path = await open({ multiple: false, filters: [{ name: '音频', extensions: ['wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg'] }] })
      if (!path) return
      const filename = path.split('/').pop() ?? '音频文件'
      const session = await createCapture(`导入 · ${filename}`)
      await importAudio(session, path)
      const coreRevision = await appendTranscriptRevision(session.id, 'demo-local', [{
        id: `seg_${crypto.randomUUID().replaceAll('-', '')}`,
        start_ms: 0,
        end_ms: 4_000,
        source: 'imported',
        text: `${filename} 已安全导入。当前使用演示转写 Provider，后续可切换本地 ASR 模型重新处理。`,
      }])
      const importedRecord: EvidenceRecord = {
        id: session.id,
        title: session.title,
        startedAt: '刚刚',
        duration: '待分析',
        status: 'processing',
        originalRevision: {
          number: coreRevision.number,
          provider: '本地演示 ASR',
          label: `原始转写 · r${coreRevision.number}`,
          segments: coreRevision.segments.map((segment) => ({ id: segment.id, startMs: segment.start_ms, endMs: segment.end_ms, source: '导入音频', text: segment.text })),
        },
        revision: {
          number: coreRevision.number,
          provider: '本地演示 ASR',
          label: `原始转写 · r${coreRevision.number}`,
          segments: coreRevision.segments.map((segment) => ({ id: segment.id, startMs: segment.start_ms, endMs: segment.end_ms, source: '导入音频', text: segment.text })),
        },
      }
      setRecords((current) => [importedRecord, ...current])
      setSelectedId(importedRecord.id)
      setNotice('音频已复制到 LifeSub 本地 Evidence 目录，并完成内容校验。')
    } catch (error) {
      setNotice(`导入失败：${String(error)}`)
    }
  }

  return (
    <div className="app-shell">
      <nav className="sidebar" aria-label="主导航">
        <div className="brand"><span className="brand__mark"><AudioLines /></span><span><strong>LifeSub</strong><small>旁白</small></span></div>
        <div className="nav-items"><button className={`nav-item ${view === 'timeline' ? 'nav-item--active' : ''}`} onClick={() => setView('timeline')}><Archive size={18} />时间线</button><button className="nav-item" onClick={handleImport}><Import size={18} />导入音频</button></div>
        <button className={`nav-item nav-item--settings ${view === 'settings' ? 'nav-item--active' : ''}`} onClick={() => setView('settings')}><Settings size={18} />设置</button>
      </nav>
      <section className="workspace">
        {view === 'timeline' && <RecorderBar state={captureState} onStateChange={changeCaptureState} />}
        {notice && <div className="notice" role="status">{notice}<button aria-label="关闭提示" onClick={() => setNotice('')}>×</button></div>}
        {view === 'timeline' ? <div className="workspace__content"><RecordList records={records} selectedId={selectedId} query={query} onQueryChange={setQuery} onSelect={setSelectedId} /><TranscriptView record={selectedRecord} query={query} onRevisionChange={updateRevision} onNotice={setNotice} /></div> : <SettingsView />}
      </section>
    </div>
  )
}
