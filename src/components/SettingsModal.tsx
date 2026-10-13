import { useCallback, useEffect, useState, type KeyboardEvent, type ReactNode } from 'react'
import { Modal } from './Modal'
import { RecordingSettings } from './RecordingSettings'
import { AsrSettings } from './AsrSettings'
import { ModelManager } from './ModelManager'
import { AboutTab } from './AboutTab'
import { DiagnosticsSettings } from './DiagnosticsSettings'
import '../settings.css'

const TABS = [
  { id: 'recording', label: '录音' },
  { id: 'asr', label: 'ASR 与模型' },
  { id: 'diagnostics', label: '诊断' },
]

interface SettingsModalProps {
  open: boolean
  onClose: () => void
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState('recording')
  const [visitedTabs, setVisitedTabs] = useState(() => new Set(['recording']))
  const [dirtyTabs, setDirtyTabs] = useState<Record<string, boolean>>({})
  const [confirmDiscard, setConfirmDiscard] = useState(false)
  const [sessionKey, setSessionKey] = useState(0)
  const hasDirtyTabs = Object.values(dirtyTabs).some(Boolean)

  useEffect(() => {
    if (open) {
      setActiveTab('recording')
      setVisitedTabs(new Set(['recording']))
      setDirtyTabs({})
      setConfirmDiscard(false)
    }
  }, [open])

  const selectTab = (tabId: string) => {
    setActiveTab(tabId)
    setVisitedTabs((current) => new Set(current).add(tabId))
  }

  const reportDirty = useCallback((tabId: string, dirty: boolean) => {
    setDirtyTabs((current) => current[tabId] === dirty ? current : { ...current, [tabId]: dirty })
  }, [])

  const requestClose = () => {
    if (hasDirtyTabs) {
      setConfirmDiscard(true)
      return
    }
    onClose()
  }

  const discardAndClose = () => {
    setConfirmDiscard(false)
    setDirtyTabs({})
    setSessionKey((current) => current + 1)
    onClose()
  }

  const handleTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, currentIndex: number) => {
    let nextIndex = currentIndex
    if (event.key === 'ArrowDown' || event.key === 'ArrowRight') nextIndex = (currentIndex + 1) % TABS.length
    if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') nextIndex = (currentIndex - 1 + TABS.length) % TABS.length
    if (event.key === 'Home') nextIndex = 0
    if (event.key === 'End') nextIndex = TABS.length - 1
    if (nextIndex === currentIndex) return

    event.preventDefault()
    const nextTab = TABS[nextIndex]
    selectTab(nextTab.id)
    document.getElementById(`${nextTab.id}-tab`) instanceof HTMLButtonElement
      && (document.getElementById(`${nextTab.id}-tab`) as HTMLButtonElement).focus()
  }

  return (
    <Modal open={open} onClose={requestClose} title="设置" bodyClassName="settings-modal__body" panelClassName="settings-modal__panel">
      <div className="settings-layout" key={sessionKey}>
        <nav className="settings-nav" aria-label="设置分组">
          <div className="settings-nav__list" role="tablist">
          {TABS.map((tab, index) => (
            <button
              key={tab.id}
              id={`${tab.id}-tab`}
              role="tab"
              type="button"
              tabIndex={tab.id === activeTab ? 0 : -1}
              aria-selected={tab.id === activeTab}
              aria-controls={`${tab.id}-panel`}
              className={`settings-nav__item ${tab.id === activeTab ? 'settings-nav__item--active' : ''}`}
              onClick={() => selectTab(tab.id)}
              onKeyDown={(event) => handleTabKeyDown(event, index)}
            >
              <span>{tab.label}</span>
              {dirtyTabs[tab.id] && <span className="settings-nav__dirty" aria-label="未保存">•</span>}
            </button>
          ))}
          </div>
        </nav>
        <div className="settings-content">
          {visitedTabs.has('recording') && <SettingsPanel id="recording" activeTab={activeTab}><RecordingSettings onDirtyChange={(dirty) => reportDirty('recording', dirty)} /></SettingsPanel>}
          {visitedTabs.has('asr') && <SettingsPanel id="asr" activeTab={activeTab}><div className="settings-panel__stack"><AsrSettings onDirtyChange={(dirty) => reportDirty('asr', dirty)} /><ModelManager /></div></SettingsPanel>}
          {visitedTabs.has('diagnostics') && <SettingsPanel id="diagnostics" activeTab={activeTab}><div className="settings-panel__stack"><DiagnosticsSettings /><AboutTab /></div></SettingsPanel>}
        </div>
      </div>
      <Modal
        open={confirmDiscard}
        onClose={() => setConfirmDiscard(false)}
        title="放弃未保存的更改？"
        role="alertdialog"
        closeLabel="关闭放弃更改确认"
        bodyClassName="settings-discard-body"
        panelClassName="settings-discard-panel"
      >
        <p>当前设置尚未保存。放弃后，本次修改不会生效。</p>
        <div className="settings-actions settings-discard-actions">
          <button type="button" className="button" onClick={() => setConfirmDiscard(false)}>继续编辑</button>
          <button type="button" className="button button--danger" onClick={discardAndClose}>放弃更改</button>
        </div>
      </Modal>
    </Modal>
  )
}

function SettingsPanel({ id, activeTab, children }: { id: string; activeTab: string; children: ReactNode }) {
  const active = id === activeTab
  return (
    <section id={`${id}-panel`} role="tabpanel" aria-labelledby={`${id}-tab`} hidden={!active} className="settings-panel">
      {children}
    </section>
  )
}
