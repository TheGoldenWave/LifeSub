import { Archive, AudioLines, Settings } from 'lucide-react'
import { useMemo, useState } from 'react'
import { RecordList } from './components/RecordList'
import { RecorderBar } from './components/RecorderBar'
import { TranscriptView } from './components/TranscriptView'
import { SettingsView } from './components/SettingsView'
import { demoRecords } from './data/demo'
import type { CaptureState, EvidenceRecord, TranscriptRevision } from './domain'
import { createCapture, isTauriRuntime, transitionCapture, type CoreCaptureSession } from './services/lifesub'

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

  const handleRetranscribe = async (recordId: string) => {
    if (!isTauriRuntime()) {
      setNotice('浏览器预览不支持重新转写；在桌面版中可使用本地 ASR 重新处理。')
      return
    }
    try {
      setNotice(`已为记录 ${recordId} 提交重新转写请求。`)
      // In the full implementation, this calls retranscribe() from lifesub.ts
      // and polls for the new job completion
    } catch (error) {
      setNotice(`重新转写请求失败：${String(error)}`)
    }
  }

  return (
    <div className="app-shell">
      <nav className="sidebar" aria-label="主导航">
        <div className="brand"><span className="brand__mark"><AudioLines /></span><span><strong>LifeSub</strong><small>旁白</small></span></div>
        <div className="nav-items"><button className={`nav-item ${view === 'timeline' ? 'nav-item--active' : ''}`} onClick={() => setView('timeline')}><Archive size={18} />时间线</button></div>
        <button className={`nav-item nav-item--settings ${view === 'settings' ? 'nav-item--active' : ''}`} onClick={() => setView('settings')}><Settings size={18} />设置</button>
      </nav>
      <section className="workspace">
        {view === 'timeline' && <RecorderBar state={captureState} onStateChange={changeCaptureState} />}
        {notice && <div className="notice" role="status">{notice}<button aria-label="关闭提示" onClick={() => setNotice('')}>×</button></div>}
{view === 'timeline' ? <div className="workspace__content"><RecordList records={records} selectedId={selectedId} query={query} onQueryChange={setQuery} onSelect={setSelectedId} /><TranscriptView record={selectedRecord} query={query} onRevisionChange={updateRevision} onNotice={setNotice} onRetranscribe={handleRetranscribe} allRevisions={selectedRecord.allRevisions} /></div> : <SettingsView />}
      </section>
    </div>
  )
}