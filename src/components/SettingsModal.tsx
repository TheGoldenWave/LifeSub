import { useState } from 'react'
import { Modal } from './Modal'
import { RecordingSettings } from './RecordingSettings'
import { AsrSettings } from './AsrSettings'
import { ModelManager } from './ModelManager'
import { AboutTab } from './AboutTab'

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

  return (
    <Modal open={open} onClose={onClose} title="设置">
      <div className="settings-layout">
        <nav className="settings-nav">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              className={`settings-nav__item ${tab.id === activeTab ? 'settings-nav__item--active' : ''}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="settings-content">
          {activeTab === 'recording' && <RecordingSettings />}
          {activeTab === 'asr' && <AsrSettings />}
          {activeTab === 'models' && <ModelManager />}
          {activeTab === 'about' && <AboutTab />}
        </div>
      </div>
    </Modal>
  )
}