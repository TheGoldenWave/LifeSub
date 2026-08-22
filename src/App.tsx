import { useEffect, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { Sidebar, type PageId } from './components/Sidebar'
import { LiveCapture } from './components/LiveCapture'
import { TimelineView } from './components/TimelineView'
import { DictionaryView } from './components/DictionaryView'
import { SettingsModal } from './components/SettingsModal'
import { demoRecords } from './data/demo'
import { importAudioRecord, loadTimelineRecords } from './data/adapter'
import { getAcceptanceScenario, recordHeartbeat } from './acceptance'
import { isTauriRuntime } from './services/lifesub'
import type { EvidenceRecord } from './domain'

export default function App() {
  const [activePage, setActivePage] = useState<PageId>('live')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [records, setRecords] = useState<EvidenceRecord[]>(() => demoModeValue() ? demoRecords : [])
  const [notice, setNotice] = useState('')
  const [isImporting, setIsImporting] = useState(false)
  const [timelineLoading, setTimelineLoading] = useState(!demoModeValue())
  const [timelineError, setTimelineError] = useState('')
  const demoMode = !isTauriRuntime()

  useEffect(() => {
    const scenario = getAcceptanceScenario()
    if (scenario) {
      recordHeartbeat(scenario)
    }
  }, [])

  const refreshTimeline = async () => {
    if (demoMode) return
    setTimelineLoading(true)
    setTimelineError('')
    try {
      setRecords(await loadTimelineRecords())
    } catch (error) {
      setRecords([])
      setTimelineError(error instanceof Error ? error.message : '时间线加载失败')
    } finally {
      setTimelineLoading(false)
    }
  }

  useEffect(() => {
    void refreshTimeline()
  }, [demoMode])

  const handleImportAudio = async () => {
    if (demoMode) {
      setActivePage('timeline')
      setNotice('浏览器演示模式仅支持示例数据，请在桌面版中导入真实音频。')
      return
    }
    if (isImporting) return

    setIsImporting(true)
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Audio',
          extensions: ['wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg'],
        }],
      })
      if (typeof selected !== 'string' || !selected) return
      const title = selected.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') || '导入音频'
      const outcome = await importAudioRecord(selected, title)
      await refreshTimeline()
      setActivePage('timeline')
      setNotice(outcome?.asr_warning
        ? `音频已保存；自动转写未启动：${outcome.asr_warning}`
        : '已导入音频，记录已写入 Catalog。')
    } catch (error) {
      const message = error instanceof Error ? error.message : '未知错误'
      setNotice(`导入失败：${message}`)
    } finally {
      setIsImporting(false)
    }
  }

  return (
    <div className="app-shell">
      <Sidebar
        activePage={activePage}
        onNavigate={setActivePage}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <section className="workspace">
        {demoMode && (
          <div className="notice" role="status">
            浏览器演示模式
          </div>
        )}
        {notice && (
          <div className="notice" role="status">
            {notice}
            <button aria-label="关闭提示" onClick={() => setNotice('')}>×</button>
          </div>
        )}
        {activePage === 'live' && <LiveCapture onNotice={setNotice} />}
        {activePage === 'timeline' && (
          <TimelineView
            records={records}
            onRecordsChange={setRecords}
            onNotice={setNotice}
            onImportAudio={() => void handleImportAudio()}
            loading={timelineLoading}
            error={timelineError}
            onRetry={() => void refreshTimeline()}
          />
        )}
        {activePage === 'dictionary' && <DictionaryView onNotice={setNotice} />}
      </section>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  )
}

function demoModeValue() {
  return !isTauriRuntime()
}
