interface TabBarProps {
  tabs: { id: string; label: string }[]
  activeTab: string
  onSelect: (id: string) => void
}

export function TabBar({ tabs, activeTab, onSelect }: TabBarProps) {
  return (
    <nav className="tab-bar" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={tab.id === activeTab}
          className={`tab-bar__tab ${tab.id === activeTab ? 'tab-bar__tab--active' : ''}`}
          onClick={() => onSelect(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </nav>
  )
}