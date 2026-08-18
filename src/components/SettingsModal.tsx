import { Modal } from './Modal'

interface SettingsModalProps {
  open: boolean
  onClose: () => void
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  return (
    <Modal open={open} onClose={onClose} title="设置">
      <div className="settings-layout">
        <nav className="settings-nav">
          <button className="settings-nav__item settings-nav__item--active">录音设置</button>
          <button className="settings-nav__item">ASR 设置</button>
          <button className="settings-nav__item">模型</button>
          <button className="settings-nav__item">关于</button>
        </nav>
        <div className="settings-content">
          <span className="eyebrow">RECORDING</span>
          <h1>录音设置</h1>
          <p>设置将在后续 Task 中实现完整交互。</p>
        </div>
      </div>
    </Modal>
  )
}