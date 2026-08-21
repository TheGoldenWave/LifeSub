import { useEffect, useMemo, useState } from 'react'
import { Check, Plus, Search, X } from 'lucide-react'
import type { DictionaryCategory, DictionaryEntry } from '../domain'
import {
  createCategoryAdapter,
  createEntryAdapter,
  deleteCategoryAdapter,
  deleteEntryAdapter,
  loadCategories,
  loadEntries,
  toggleEntryAdapter,
  updateEntryAdapter,
} from '../data/adapter'

interface DictionaryViewProps {
  onNotice: (msg: string) => void
}

type ScopeFilter = 'global' | 'project'
type DetailMode = 'empty' | 'entry-view' | 'entry-create' | 'entry-edit' | 'category-create'
type SaveState = 'idle' | 'saving' | 'error'
type LoadState = 'idle' | 'loading' | 'ready' | 'error'

interface EntryFormState {
  term: string
  pinyin: string
  aliases: string
  note: string
}

const PROJECT_SCOPE_ID = 'project:lifesub'
const EMPTY_ENTRY_FORM: EntryFormState = {
  term: '',
  pinyin: '',
  aliases: '',
  note: '',
}

function scopeMatches(scopeFilter: ScopeFilter, category: DictionaryCategory) {
  return scopeFilter === 'global' ? category.scope === 'global' : category.scope.startsWith('project:')
}

function currentScopeValue(scopeFilter: ScopeFilter) {
  return scopeFilter === 'global' ? 'global' : PROJECT_SCOPE_ID
}

export function DictionaryView({ onNotice }: DictionaryViewProps) {
  const [categories, setCategories] = useState<DictionaryCategory[]>([])
  const [entries, setEntries] = useState<DictionaryEntry[]>([])
  const [scope, setScope] = useState<ScopeFilter>('global')
  const [selectedCategoryId, setSelectedCategoryId] = useState('')
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null)
  const [searchTerm, setSearchTerm] = useState('')
  const [detailMode, setDetailMode] = useState<DetailMode>('empty')
  const [categoriesLoadState, setCategoriesLoadState] = useState<LoadState>('loading')
  const [entriesLoadState, setEntriesLoadState] = useState<LoadState>('idle')
  const [categoriesLoadError, setCategoriesLoadError] = useState('')
  const [entriesLoadError, setEntriesLoadError] = useState('')
  const [categoriesReloadKey, setCategoriesReloadKey] = useState(0)
  const [entriesReloadKey, setEntriesReloadKey] = useState(0)

  const [categoryName, setCategoryName] = useState('')
  const [categoryValidationError, setCategoryValidationError] = useState('')
  const [categorySaveState, setCategorySaveState] = useState<SaveState>('idle')
  const [categorySaveError, setCategorySaveError] = useState('')

  const [entryForm, setEntryForm] = useState<EntryFormState>(EMPTY_ENTRY_FORM)
  const [entryValidationError, setEntryValidationError] = useState('')
  const [entrySaveState, setEntrySaveState] = useState<SaveState>('idle')
  const [entrySaveError, setEntrySaveError] = useState('')

  useEffect(() => {
    let active = true

    setCategoriesLoadState('loading')
    setCategoriesLoadError('')

    void loadCategories()
      .then((loadedCategories) => {
        if (!active) return
        setCategories(loadedCategories)
        setCategoriesLoadState('ready')
      })
      .catch(() => {
        if (!active) return
        setCategories([])
        setCategoriesLoadState('error')
        setCategoriesLoadError('分类加载失败，请重试。')
      })

    return () => {
      active = false
    }
  }, [categoriesReloadKey])

  const filteredCategories = useMemo(
    () => categories.filter((category) => scopeMatches(scope, category)),
    [categories, scope],
  )

  const selectedCategory = filteredCategories.find((category) => category.id === selectedCategoryId) ?? null

  useEffect(() => {
    if (filteredCategories.length === 0) {
      setSelectedCategoryId('')
      setSelectedEntryId(null)
      setEntries([])
      setEntriesLoadState('idle')
      setEntriesLoadError('')
      if (detailMode !== 'category-create') {
        setDetailMode('empty')
      }
      return
    }

    const categoryStillVisible = filteredCategories.some((category) => category.id === selectedCategoryId)
    if (!categoryStillVisible) {
      setSelectedCategoryId(filteredCategories[0].id)
      setSelectedEntryId(null)
      if (detailMode !== 'category-create') {
        setDetailMode('empty')
      }
    }
  }, [detailMode, filteredCategories, selectedCategoryId])

  useEffect(() => {
    if (!selectedCategoryId) {
      setEntries([])
      setEntriesLoadState('idle')
      setEntriesLoadError('')
      return
    }

    let active = true

    setEntries([])
    setEntriesLoadState('loading')
    setEntriesLoadError('')

    void loadEntries(selectedCategoryId)
      .then((loadedEntries) => {
        if (!active) return
        setEntries(loadedEntries)
        setEntriesLoadState('ready')
      })
      .catch(() => {
        if (!active) return
        setEntries([])
        setEntriesLoadState('error')
        setEntriesLoadError('词条加载失败，请重试。')
      })

    return () => {
      active = false
    }
  }, [entriesReloadKey, selectedCategoryId])

  const selectedEntry = entries.find((entry) => entry.id === selectedEntryId) ?? null
  const visibleEntries = entries.filter((entry) => {
    if (!searchTerm) return true
    return entry.term.includes(searchTerm) || entry.pinyin.includes(searchTerm) || entry.aliases.includes(searchTerm)
  })

  const currentScopeLabel = scope === 'global' ? '全局词典' : '项目补充词典'
  const categoryActionDisabled = categorySaveState === 'saving'
  const entryActionDisabled = entrySaveState === 'saving'

  function resetCategoryForm() {
    setCategoryName('')
    setCategoryValidationError('')
    setCategorySaveError('')
    setCategorySaveState('idle')
  }

  function resetEntryForm(nextForm: EntryFormState = EMPTY_ENTRY_FORM) {
    setEntryForm(nextForm)
    setEntryValidationError('')
    setEntrySaveError('')
    setEntrySaveState('idle')
  }

  function openCategoryCreateForm() {
    resetCategoryForm()
    setSelectedEntryId(null)
    setDetailMode('category-create')
  }

  function openEntryCreateForm() {
    if (!selectedCategoryId || entriesLoadState !== 'ready') return
    resetEntryForm()
    setSelectedEntryId(null)
    setDetailMode('entry-create')
  }

  function openEntryEditForm() {
    if (!selectedEntry) return
    resetEntryForm({
      term: selectedEntry.term,
      pinyin: selectedEntry.pinyin,
      aliases: selectedEntry.aliases,
      note: selectedEntry.note,
    })
    setDetailMode('entry-edit')
  }

  function closeDetailPanel() {
    resetCategoryForm()
    resetEntryForm()
    setDetailMode(selectedEntryId ? 'entry-view' : 'empty')
  }

  function retryCategoriesLoad() {
    setCategoriesReloadKey((previousKey) => previousKey + 1)
  }

  function retryEntriesLoad() {
    if (!selectedCategoryId) return
    setEntriesReloadKey((previousKey) => previousKey + 1)
  }

  function updateCategoryEntryCount(categoryId: string, delta: number) {
    setCategories((previousCategories) =>
      previousCategories.map((category) =>
        category.id === categoryId
          ? { ...category, entryCount: Math.max(0, category.entryCount + delta) }
          : category,
      ),
    )
  }

  async function handleSaveCategory() {
    const trimmedName = categoryName.trim()
    if (!trimmedName) {
      setCategoryValidationError('分类名称不能为空')
      return
    }

    setCategoryValidationError('')
    setCategorySaveError('')
    setCategorySaveState('saving')

    try {
      const createdCategory = await createCategoryAdapter(trimmedName, currentScopeValue(scope))
      setCategories((previousCategories) => [...previousCategories, createdCategory])
      setSelectedCategoryId(createdCategory.id)
      setDetailMode('empty')
      resetCategoryForm()
      onNotice(`分类「${trimmedName}」已创建`)
    } catch {
      setCategorySaveState('error')
      setCategorySaveError('保存失败，请重试。')
    }
  }

  async function handleDeleteCategory() {
    if (!selectedCategoryId) return
    if (!window.confirm('确定要删除此分类及其所有词条？')) return

    try {
      await deleteCategoryAdapter(selectedCategoryId)
      setCategories((previousCategories) => previousCategories.filter((category) => category.id !== selectedCategoryId))
      setSelectedEntryId(null)
      setEntries([])
      setDetailMode('empty')
      onNotice('分类已删除')
    } catch {
      onNotice('分类删除失败，请重试')
    }
  }

  async function handleSaveEntry() {
    if (!selectedCategoryId) {
      setEntrySaveState('error')
      setEntrySaveError('请先创建或选择分类。')
      return
    }

    const trimmedTerm = entryForm.term.trim()
    if (!trimmedTerm) {
      setEntryValidationError('标准词条不能为空')
      return
    }

    setEntryValidationError('')
    setEntrySaveError('')
    setEntrySaveState('saving')

    try {
      if (detailMode === 'entry-create') {
        const createdEntry = await createEntryAdapter(
          selectedCategoryId,
          trimmedTerm,
          entryForm.pinyin.trim(),
          entryForm.aliases.trim(),
          entryForm.note.trim(),
        )
        setEntries((previousEntries) => [...previousEntries, createdEntry])
        updateCategoryEntryCount(selectedCategoryId, 1)
        setSelectedEntryId(createdEntry.id)
        setDetailMode('entry-view')
        resetEntryForm()
        onNotice(`词条「${trimmedTerm}」已创建`)
        return
      }

      if (!selectedEntry) return

      await updateEntryAdapter(
        selectedEntry.id,
        trimmedTerm,
        entryForm.pinyin.trim(),
        entryForm.aliases.trim(),
        entryForm.note.trim(),
      )

      setEntries((previousEntries) =>
        previousEntries.map((entry) =>
          entry.id === selectedEntry.id
            ? {
                ...entry,
                term: trimmedTerm,
                pinyin: entryForm.pinyin.trim(),
                aliases: entryForm.aliases.trim(),
                note: entryForm.note.trim(),
              }
            : entry,
        ),
      )
      setDetailMode('entry-view')
      setEntrySaveState('idle')
      onNotice(`词条「${trimmedTerm}」已保存`)
    } catch {
      setEntrySaveState('error')
      setEntrySaveError('保存失败，请重试。')
    }
  }

  async function handleToggleEntry() {
    if (!selectedEntry) return

    try {
      await toggleEntryAdapter(selectedEntry.id, !selectedEntry.enabled)
      setEntries((previousEntries) =>
        previousEntries.map((entry) =>
          entry.id === selectedEntry.id ? { ...entry, enabled: !entry.enabled } : entry,
        ),
      )
    } catch {
      onNotice('词条状态更新失败，请重试')
    }
  }

  async function handleDeleteEntry() {
    if (!selectedEntry) return
    if (!window.confirm(`确定删除词条「${selectedEntry.term}」？`)) return

    try {
      await deleteEntryAdapter(selectedEntry.id)
      setEntries((previousEntries) => previousEntries.filter((entry) => entry.id !== selectedEntry.id))
      updateCategoryEntryCount(selectedEntry.categoryId, -1)
      setSelectedEntryId(null)
      setDetailMode('empty')
      onNotice(`词条「${selectedEntry.term}」已删除`)
    } catch {
      onNotice('词条删除失败，请重试')
    }
  }

  if (categoriesLoadState === 'loading') {
    return (
      <main className="dictionary-view">
        <p className="empty-state">加载中...</p>
      </main>
    )
  }

  return (
    <main className="dictionary-view">
      <header className="dictionary-view__header">
        <div className="dictionary-view__title-group">
          <span className="eyebrow">DICTIONARY</span>
          <h1>常用词库</h1>
          <div className="dictionary-view__scope-group">
            <span className="dictionary-view__scope-label">当前范围：</span>
            <select
              aria-label="词典范围"
              className="dictionary-view__scope"
              value={scope}
              onChange={(event) => {
                setScope(event.target.value as ScopeFilter)
                setSelectedEntryId(null)
                setDetailMode('empty')
              }}
            >
              <option value="global">全局词典</option>
              <option value="project">项目补充</option>
            </select>
          </div>
        </div>
        <div className="dictionary-view__actions">
          <button className="text-button" disabled={categoryActionDisabled} onClick={openCategoryCreateForm}>
            <Plus size={14} />
            新建分类
          </button>
          <button
            className="text-button"
            disabled={!selectedCategoryId || entriesLoadState !== 'ready' || entryActionDisabled || filteredCategories.length === 0}
            onClick={openEntryCreateForm}
            title={filteredCategories.length === 0 ? '请先创建分类' : undefined}
          >
            <Plus size={14} />
            新建词条
          </button>
          {selectedCategoryId && (
            <button className="text-button" onClick={handleDeleteCategory}>
              删除分类
            </button>
          )}
        </div>
      </header>

      <div className="dictionary-view__body">
        <aside className="dictionary-view__categories">
          {categoriesLoadState === 'error' ? (
            <div className="empty-state">
              <p>{categoriesLoadError}</p>
              <button className="text-button" onClick={retryCategoriesLoad}>
                重试加载分类
              </button>
            </div>
          ) : filteredCategories.length === 0 ? (
            <div className="empty-state">
              <p>当前范围暂无分类</p>
              <p>先新建分类，再维护会影响未来任务的词条。</p>
              <button className="text-button" onClick={openCategoryCreateForm}>
                新建当前范围分类
              </button>
            </div>
          ) : (
            filteredCategories.map((category) => (
              <button
                key={category.id}
                className={`dictionary-category ${category.id === selectedCategoryId ? 'dictionary-category--active' : ''}`}
                onClick={() => {
                  setSelectedCategoryId(category.id)
                  setEntries([])
                  setEntriesLoadState('loading')
                  setEntriesLoadError('')
                  setSelectedEntryId(null)
                  setDetailMode('empty')
                }}
              >
                <span className="dictionary-category__name">{category.name}</span>
                <span className="dictionary-category__count">{category.entryCount} 个词</span>
              </button>
            ))
          )}
        </aside>

        <section className="dictionary-view__entries">
          <div className="search-field" style={{ marginBottom: 'var(--spacing-3)' }}>
            <Search size={16} />
            <input
              type="search"
              placeholder="搜索词条..."
              value={searchTerm}
              onChange={(event) => setSearchTerm(event.target.value)}
            />
          </div>

          {entriesLoadState === 'loading' && selectedCategory ? (
            <p className="empty-state">词条加载中...</p>
          ) : entriesLoadState === 'error' && selectedCategory ? (
            <div className="empty-state">
              <p>{entriesLoadError}</p>
              <button className="text-button" onClick={retryEntriesLoad}>
                重试加载词条
              </button>
            </div>
          ) : selectedCategory ? (
            <div className="dictionary-entries">
              {visibleEntries.map((entry) => (
                <button
                  key={entry.id}
                  aria-label={entry.term}
                  className={`dictionary-entry ${entry.id === selectedEntryId ? 'dictionary-entry--active' : ''}`}
                  onClick={() => {
                    setSelectedEntryId(entry.id)
                    setDetailMode('entry-view')
                  }}
                >
                  <span className="dictionary-entry__status">
                    {entry.enabled ? <Check size={12} /> : <X size={12} />}
                  </span>
                  <span className="dictionary-entry__term">{entry.term}</span>
                  {entry.aliases && <span className="dictionary-entry__aliases">{entry.aliases}</span>}
                </button>
              ))}
              {visibleEntries.length === 0 && (
                <p className="empty-state">暂无词条，点击「新建词条」添加。</p>
              )}
            </div>
          ) : (
            <p className="empty-state">请先选择或创建分类，再添加词条。</p>
          )}
        </section>

        <aside className="dictionary-view__detail">
          {detailMode === 'category-create' && (
            <>
              <h3>新建分类</h3>
              <div className="dictionary-detail__fields">
                <div className="dictionary-detail__field">
                  <label htmlFor="dictionary-category-name">分类名称</label>
                  <input
                    id="dictionary-category-name"
                    value={categoryName}
                    onChange={(event) => {
                      setCategoryName(event.target.value)
                      if (categoryValidationError) setCategoryValidationError('')
                    }}
                  />
                </div>
                <div className="dictionary-detail__field">
                  <label>生效范围</label>
                  <span>{currentScopeLabel}</span>
                </div>
              </div>
              {categoryValidationError && <p className="empty-state">{categoryValidationError}</p>}
              {categorySaveState === 'saving' && <p className="empty-state">保存中...</p>}
              {categorySaveError && <p className="empty-state">{categorySaveError}</p>}
              <div className="dictionary-detail__actions">
                <button className="text-button" disabled={categoryActionDisabled} onClick={handleSaveCategory}>
                  保存分类
                </button>
                {categorySaveState === 'error' && (
                  <button className="text-button" onClick={handleSaveCategory}>
                    重试保存
                  </button>
                )}
                <button className="text-button" disabled={categoryActionDisabled} onClick={closeDetailPanel}>
                  取消
                </button>
              </div>
            </>
          )}

          {(detailMode === 'entry-create' || detailMode === 'entry-edit') && (
            <>
              <h3>{detailMode === 'entry-create' ? '新建词条' : '编辑词条'}</h3>
              <div className="dictionary-detail__fields">
                <div className="dictionary-detail__field">
                  <label htmlFor="dictionary-entry-term">标准词条</label>
                  <input
                    id="dictionary-entry-term"
                    value={entryForm.term}
                    onChange={(event) => {
                      setEntryForm((previous) => ({ ...previous, term: event.target.value }))
                      if (entryValidationError) setEntryValidationError('')
                    }}
                  />
                </div>
                <div className="dictionary-detail__field">
                  <label htmlFor="dictionary-entry-pinyin">拼音</label>
                  <input
                    id="dictionary-entry-pinyin"
                    value={entryForm.pinyin}
                    onChange={(event) => setEntryForm((previous) => ({ ...previous, pinyin: event.target.value }))}
                  />
                </div>
                <div className="dictionary-detail__field">
                  <label htmlFor="dictionary-entry-aliases">别名</label>
                  <input
                    id="dictionary-entry-aliases"
                    value={entryForm.aliases}
                    onChange={(event) => setEntryForm((previous) => ({ ...previous, aliases: event.target.value }))}
                  />
                </div>
                <div className="dictionary-detail__field">
                  <label htmlFor="dictionary-entry-note">备注</label>
                  <input
                    id="dictionary-entry-note"
                    value={entryForm.note}
                    onChange={(event) => setEntryForm((previous) => ({ ...previous, note: event.target.value }))}
                  />
                </div>
                <div className="dictionary-detail__field">
                  <label>所属分类</label>
                  <span>{selectedCategory?.name ?? '未选择分类'}</span>
                </div>
                {detailMode === 'entry-edit' && (
                  <div className="dictionary-detail__field">
                    <label>分类调整</label>
                    <span>当前版本暂不支持修改所属分类；如需迁移，请在目标分类新建后删除旧词条。</span>
                  </div>
                )}
              </div>
              {entryValidationError && <p className="empty-state">{entryValidationError}</p>}
              {entrySaveState === 'saving' && <p className="empty-state">保存中...</p>}
              {entrySaveError && <p className="empty-state">{entrySaveError}</p>}
              <div className="dictionary-detail__actions">
                <button className="text-button" disabled={entryActionDisabled} onClick={handleSaveEntry}>
                  保存词条
                </button>
                {entrySaveState === 'error' && (
                  <button className="text-button" onClick={handleSaveEntry}>
                    重试保存
                  </button>
                )}
                <button className="text-button" disabled={entryActionDisabled} onClick={closeDetailPanel}>
                  取消
                </button>
              </div>
            </>
          )}

          {detailMode === 'entry-view' && selectedEntry && (
            <>
              <h3>{selectedEntry.term}</h3>
              <div className="dictionary-detail__fields">
                <div className="dictionary-detail__field">
                  <label>分类</label>
                  <span>{selectedCategory?.name ?? '—'}</span>
                </div>
                <div className="dictionary-detail__field">
                  <label>状态</label>
                  <span className={`status-pill ${selectedEntry.enabled ? '' : 'status-pill--quiet'}`}>
                    {selectedEntry.enabled ? '启用' : '停用'}
                  </span>
                </div>
                <div className="dictionary-detail__field">
                  <label>拼音</label>
                  <span>{selectedEntry.pinyin || '—'}</span>
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
                <button className="text-button" onClick={openEntryEditForm}>
                  编辑词条
                </button>
                <button className="text-button" onClick={handleToggleEntry}>
                  {selectedEntry.enabled ? '停用' : '启用'}
                </button>
                <button className="text-button" onClick={handleDeleteEntry}>
                  删除
                </button>
              </div>
            </>
          )}

          {detailMode === 'empty' && (
            <>
              <h3>词典说明</h3>
              <div className="dictionary-detail__fields">
                <div className="dictionary-detail__field">
                  <label>当前范围</label>
                  <span>{currentScopeLabel}</span>
                </div>
                <div className="dictionary-detail__field">
                  <label>作用边界</label>
                  <span>词典会影响未来任务中的 ASR 修正，不会回写或覆盖历史转写记录。</span>
                </div>
                <div className="dictionary-detail__field">
                  <label>操作建议</label>
                  <span>
                    先在左侧选择分类，再新建词条；如果当前范围没有分类，请先创建分类后继续。
                  </span>
                </div>
                <div className="dictionary-detail__field">
                  <label>工作原理</label>
                  <span>ASR 识别到词库中的词条时，会在后续任务中优先修正为正确写法；历史转写保持原样。</span>
                </div>
              </div>
            </>
          )}
        </aside>
      </div>
    </main>
  )
}
