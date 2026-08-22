import { useEffect, useState, type KeyboardEvent } from 'react'
import { Modal } from './Modal'
import { RecordingSettings } from './RecordingSettings'
import { AsrSettings } from './AsrSettings'
import { ModelManager } from './ModelManager'
import { AboutTab } from './AboutTab'
import '../settings.css'

const TABS = [
  { id: 'recording', label: '录音设置' },
  { id: 'asr', label: 'ASR 设置' },
  { id: 'models', label: '模型' },
  { id: 'about', label: '关于' },
]

interface SettingsModalProps {
  open: boolean
  onClose: () => void
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState('recording')
  const activePanelId = `${activeTab}-panel`

  useEffect(() => {
    if (open) setActiveTab('recording')
  }, [open])

  const handleTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, currentIndex: number) => {
    let nextIndex = currentIndex
    if (event.key === 'ArrowDown' || event.key === 'ArrowRight') nextIndex = (currentIndex + 1) % TABS.length
    if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') nextIndex = (currentIndex - 1 + TABS.length) % TABS.length
    if (event.key === 'Home') nextIndex = 0
    if (event.key === 'End') nextIndex = TABS.length - 1
    if (nextIndex === currentIndex) return

    event.preventDefault()
    const nextTab = TABS[nextIndex]
    setActiveTab(nextTab.id)
    document.getElementById(`${nextTab.id}-tab`) instanceof HTMLButtonElement
      && (document.getElementById(`${nextTab.id}-tab`) as HTMLButtonElement).focus()
  }

  return (
    <Modal open={open} onClose={onClose} title="设置" bodyClassName="settings-modal__body" panelClassName="settings-modal__panel">
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分组">
          <div className="settings-nav__list" role="tablist" aria-orientation="vertical">
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
              onClick={() => setActiveTab(tab.id)}
              onKeyDown={(event) => handleTabKeyDown(event, index)}
            >
              {tab.label}
            </button>
          ))}
          </div>
        </nav>
        <div id={activePanelId} role="tabpanel" aria-labelledby={`${activeTab}-tab`} className="settings-content">
          {activeTab === 'recording' && <RecordingSettings />}
          {activeTab === 'asr' && <AsrSettings />}
          {activeTab === 'models' && <ModelManager />}
          {activeTab === 'about' && <AboutTab />}
        </div>
      </div>
    </Modal>
  )
}
