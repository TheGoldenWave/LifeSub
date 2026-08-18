import { useState } from 'react'
import { Sidebar, type PageId } from './components/Sidebar'
import { LiveCapture } from './components/LiveCapture'
import { TimelineView } from './components/TimelineView'
import { DictionaryView } from './components/DictionaryView'
import { SettingsModal } from './components/SettingsModal'
import { demoRecords, demoCategories, demoEntries } from './data/demo'
import type { EvidenceRecord } from './domain'

export default function App() {
  const [activePage, setActivePage] = useState<PageId>('live')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [records, setRecords] = useState<EvidenceRecord[]>(demoRecords)
  const [notice, setNotice] = useState('')

  const handleImportAudio = () => {
    setNotice('导入音频功能将在时间线页面中可用。')
  }

  return (
    <div className="app-shell">
      <Sidebar
        activePage={activePage}
        onNavigate={setActivePage}
        onImportAudio={handleImportAudio}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <section className="workspace">
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
          />
        )}
        {activePage === 'dictionary' && (
          <DictionaryView
            categories={demoCategories}
            entries={demoEntries}
            onNotice={setNotice}
          />
        )}
      </section>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  )
}