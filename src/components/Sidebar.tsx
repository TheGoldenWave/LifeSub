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
        <span><strong>LifeSub</strong><small>旁白</small></span>
      </div>
      <div className="nav-items">
        <button
          className={`nav-item ${activePage === 'live' ? 'nav-item--active' : ''}`}
          onClick={() => onNavigate('live')}
        >
          <Mic size={18} />录音
        </button>
        <button
          className={`nav-item ${activePage === 'timeline' ? 'nav-item--active' : ''}`}
          onClick={() => onNavigate('timeline')}
        >
          <Archive size={18} />时间线
        </button>
        <button
          className={`nav-item ${activePage === 'dictionary' ? 'nav-item--active' : ''}`}
          onClick={() => onNavigate('dictionary')}
        >
          <BookOpen size={18} />词典
        </button>
      </div>
      <button className="nav-item nav-item--settings" onClick={onOpenSettings}>
        <Settings size={18} />设置
      </button>
    </nav>
  )
}
