import { useState, useEffect } from 'react'
import { Search, Plus, Check, X } from 'lucide-react'
import type { DictionaryCategory, DictionaryEntry } from '../domain'
import {
  loadCategories,
  loadEntries,
  createCategoryAdapter,
  deleteCategoryAdapter,
  createEntryAdapter,
  updateEntryAdapter,
  toggleEntryAdapter,
  deleteEntryAdapter,
} from '../data/adapter'

interface DictionaryViewProps {
  onNotice: (msg: string) => void
}

export function DictionaryView({ onNotice }: DictionaryViewProps) {
  const [categories, setCategories] = useState<DictionaryCategory[]>([])
  const [entries, setEntries] = useState<DictionaryEntry[]>([])
  const [scope, setScope] = useState<'global' | 'project'>('global')
  const [selectedCategoryId, setSelectedCategoryId] = useState('')
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    loadCategories().then((cats) => {
      setCategories(cats)
      if (cats.length > 0 && !selectedCategoryId) setSelectedCategoryId(cats[0].id)
      setLoading(false)
    })
  }, [])

  useEffect(() => {
    if (!selectedCategoryId) return
    loadEntries(selectedCategoryId).then(setEntries)
  }, [selectedCategoryId])

  const filteredCategories = categories.filter((c) =>
    scope === 'global' ? c.scope === 'global' : c.scope.startsWith('project:')
  )

  const categoryEntries = entries.filter(
    (e) => !searchTerm || e.term.includes(searchTerm) || e.pinyin.includes(searchTerm) || e.aliases.includes(searchTerm)
  )

  const selectedEntry = entries.find((e) => e.id === selectedEntryId)

  const handleCreateCategory = async () => {
    const name = window.prompt('分类名称：')
    if (!name) return
    const cat = await createCategoryAdapter(name, scope === 'global' ? 'global' : `project:lifesub`)
    setCategories((prev) => [...prev, cat])
    setSelectedCategoryId(cat.id)
    onNotice(`分类「${name}」已创建`)
  }

  const handleDeleteCategory = async () => {
    if (!selectedCategoryId) return
    await deleteCategoryAdapter(selectedCategoryId)
    setCategories((prev) => prev.filter((c) => c.id !== selectedCategoryId))
    setSelectedCategoryId(categories[0]?.id ?? '')
    onNotice('分类已删除')
  }

  const handleCreateEntry = async () => {
    if (!selectedCategoryId) return
    const term = window.prompt('词条：')
    if (!term) return
    const entry = await createEntryAdapter(selectedCategoryId, term, '', '', '')
    setEntries((prev) => [...prev, entry])
    setSelectedEntryId(entry.id)
    onNotice(`词条「${term}」已创建`)
  }

  const handleToggleEntry = async () => {
    if (!selectedEntry) return
    await toggleEntryAdapter(selectedEntry.id, !selectedEntry.enabled)
    setEntries((prev) => prev.map((e) => e.id === selectedEntry.id ? { ...e, enabled: !e.enabled } : e))
    setSelectedEntryId(null)
  }

  const handleDeleteEntry = async () => {
    if (!selectedEntry) return
    await deleteEntryAdapter(selectedEntry.id)
    setEntries((prev) => prev.filter((e) => e.id !== selectedEntry.id))
    setSelectedEntryId(null)
    onNotice(`词条「${selectedEntry.term}」已删除`)
  }

  if (loading) return <main className="dictionary-view"><p className="empty-state">加载中...</p></main>

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
          <button className="text-button" onClick={handleCreateCategory}><Plus size={14} />新建分类</button>
          <button className="text-button" onClick={handleCreateEntry}><Plus size={14} />新建词条</button>
          {selectedCategoryId && <button className="text-button" onClick={handleDeleteCategory}>删除分类</button>}
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
              <button className="text-button" onClick={handleToggleEntry}>{selectedEntry.enabled ? '停用' : '启用'}</button>
              <button className="text-button" onClick={handleDeleteEntry}>删除</button>
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