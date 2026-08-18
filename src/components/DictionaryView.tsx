import { useState } from 'react'
import { Search, Plus, Check, X } from 'lucide-react'
import type { DictionaryCategory, DictionaryEntry } from '../domain'

interface DictionaryViewProps {
  categories: DictionaryCategory[]
  entries: DictionaryEntry[]
  onNotice: (msg: string) => void
}

export function DictionaryView({ categories, entries, onNotice: _onNotice }: DictionaryViewProps) {
  const [scope, setScope] = useState<'global' | 'project'>('global')
  const [selectedCategoryId, setSelectedCategoryId] = useState(categories[0]?.id ?? '')
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null)

  const filteredCategories = categories.filter((c) =>
    scope === 'global' ? c.scope === 'global' : c.scope.startsWith('project:')
  )

  const categoryEntries = entries.filter(
    (e) => e.categoryId === selectedCategoryId
  ).filter(
    (e) => !searchTerm || e.term.includes(searchTerm) || e.pinyin.includes(searchTerm) || e.aliases.includes(searchTerm)
  )

  const selectedEntry = entries.find((e) => e.id === selectedEntryId)

  return (
    <main className="dictionary-view">
      <header className="dictionary-view__header">
        <div>
          <span className="eyebrow">DICTIONARY</span>
          <h1>常用词库 · ASR 辅助修正</h1>
        </div>
        <div className="dictionary-view__actions">
          <select
            className="dictionary-view__scope"
            value={scope}
            onChange={(e) => setScope(e.target.value as 'global' | 'project')}
          >
            <option value="global">全局 · 默认</option>
            <option value="project">项目补充</option>
          </select>
          <button className="text-button"><Plus size={14} />新建分类</button>
          <button className="text-button"><Plus size={14} />新建词条</button>
        </div>
      </header>

      <div className="dictionary-view__body">
        <aside className="dictionary-view__categories">
          {filteredCategories.map((cat) => (
            <button
              key={cat.id}
              className={`dictionary-category ${cat.id === selectedCategoryId ? 'dictionary-category--active' : ''}`}
              onClick={() => setSelectedCategoryId(cat.id)}
            >
              <span className="dictionary-category__name">{cat.name}</span>
              <span className="dictionary-category__count">{cat.entryCount} 个词</span>
            </button>
          ))}
        </aside>

        <section className="dictionary-view__entries">
          <div className="search-field" style={{ marginBottom: 'var(--spacing-3)' }}>
            <Search size={16} />
            <input
              type="search"
              placeholder="搜索词条..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>

          <div className="dictionary-entries">
            {categoryEntries.map((entry) => (
              <button
                key={entry.id}
                className={`dictionary-entry ${entry.id === selectedEntryId ? 'dictionary-entry--active' : ''}`}
                onClick={() => setSelectedEntryId(entry.id)}
              >
                <span className="dictionary-entry__status">
                  {entry.enabled ? <Check size={12} /> : <X size={12} />}
                </span>
                <span className="dictionary-entry__term">{entry.term}</span>
                {entry.aliases && <span className="dictionary-entry__aliases">{entry.aliases}</span>}
              </button>
            ))}
            {categoryEntries.length === 0 && (
              <p className="empty-state">暂无词条，点击「新建词条」添加。</p>
            )}
          </div>
        </section>

        {selectedEntry && (
          <aside className="dictionary-view__detail">
            <h3>{selectedEntry.term}</h3>
            <div className="dictionary-detail__fields">
              <div className="dictionary-detail__field">
                <label>分类</label>
                <span>{categories.find((c) => c.id === selectedEntry.categoryId)?.name}</span>
              </div>
              <div className="dictionary-detail__field">
                <label>状态</label>
                <span className={`status-pill ${selectedEntry.enabled ? '' : 'status-pill--quiet'}`}>
                  {selectedEntry.enabled ? '启用' : '停用'}
                </span>
              </div>
              <div className="dictionary-detail__field">
                <label>拼音</label>
                <span>{selectedEntry.pinyin}</span>
              </div>
              <div className="dictionary-detail__field">
                <label>别名</label>
                <span>{selectedEntry.aliases || '—'}</span>
              </div>
              <div className="dictionary-detail__field">
                <label>备注</label>
                <span>{selectedEntry.note || '—'}</span>
              </div>
            </div>
            <div className="dictionary-detail__actions">
              <button className="text-button">编辑</button>
              <button className="text-button">{selectedEntry.enabled ? '停用' : '启用'}</button>
              <button className="text-button">删除</button>
            </div>
          </aside>
        )}
      </div>

      <footer className="dictionary-view__footer">
        💡 词典作用：ASR 识别到词库中的词条时，自动修正为正确写法。全局词典在所有会话生效，项目补充词典仅对指定项目会话生效。
      </footer>
    </main>
  )
}