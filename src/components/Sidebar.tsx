import { Mic, Archive, BookOpen, Settings, AudioLines } from 'lucide-react'

export type PageId = 'live' | 'timeline' | 'dictionary'

interface SidebarProps {
  activePage: PageId
  onNavigate: (page: PageId) => void
  onOpenSettings: () => void
}

export function Sidebar({ activePage, onNavigate, onOpenSettings }: SidebarProps) {
  return (
    <nav className="sidebar" aria-label="主导航">
      <div className="brand">
        <span className="brand__mark"><AudioLines /></span>
        <span className="brand__text"><strong>LifeSub</strong><small>旁白</small></span>
      </div>
      <div className="nav-items">
        <button
          className={`nav-item ${activePage === 'live' ? 'nav-item--active' : ''}`}
          aria-label="录音"
          aria-current={activePage === 'live' ? 'page' : undefined}
          title="录音"
          onClick={() => onNavigate('live')}
        >
          <Mic size={18} aria-hidden="true" />
          <span className="nav-item__label">录音</span>
        </button>
        <button
          className={`nav-item ${activePage === 'timeline' ? 'nav-item--active' : ''}`}
          aria-label="时间线"
          aria-current={activePage === 'timeline' ? 'page' : undefined}
          title="时间线"
          onClick={() => onNavigate('timeline')}
        >
          <Archive size={18} aria-hidden="true" />
          <span className="nav-item__label">时间线</span>
        </button>
        <button
          className={`nav-item ${activePage === 'dictionary' ? 'nav-item--active' : ''}`}
          aria-label="词典"
          aria-current={activePage === 'dictionary' ? 'page' : undefined}
          title="词典"
          onClick={() => onNavigate('dictionary')}
        >
          <BookOpen size={18} aria-hidden="true" />
          <span className="nav-item__label">词典</span>
        </button>
      </div>
      <button className="nav-item nav-item--settings" aria-label="设置" title="设置" onClick={onOpenSettings}>
        <Settings size={18} aria-hidden="true" />
        <span className="nav-item__label">设置</span>
      </button>
    </nav>
  )
}
