import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { DictionaryView } from './DictionaryView'

const adapterMocks = vi.hoisted(() => ({
  loadCategories: vi.fn(),
  loadEntries: vi.fn(),
  createCategoryAdapter: vi.fn(),
  deleteCategoryAdapter: vi.fn(),
  createEntryAdapter: vi.fn(),
  updateEntryAdapter: vi.fn(),
  toggleEntryAdapter: vi.fn(),
  deleteEntryAdapter: vi.fn(),
}))

vi.mock('../data/adapter', () => ({
  loadCategories: adapterMocks.loadCategories,
  loadEntries: adapterMocks.loadEntries,
  createCategoryAdapter: adapterMocks.createCategoryAdapter,
  deleteCategoryAdapter: adapterMocks.deleteCategoryAdapter,
  createEntryAdapter: adapterMocks.createEntryAdapter,
  updateEntryAdapter: adapterMocks.updateEntryAdapter,
  toggleEntryAdapter: adapterMocks.toggleEntryAdapter,
  deleteEntryAdapter: adapterMocks.deleteEntryAdapter,
}))

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve
    reject = innerReject
  })
  return { promise, resolve, reject }
}

describe('DictionaryView', () => {
  const onNotice = vi.fn()

  beforeEach(() => {
    vi.clearAllMocks()

    adapterMocks.loadCategories.mockResolvedValue([
      { id: 'cat-1', name: '人名', scope: 'global', entryCount: 1 },
    ])
    adapterMocks.loadEntries.mockResolvedValue([
      {
        id: 'ent-1',
        categoryId: 'cat-1',
        term: '张伟',
        pinyin: 'zhang wei',
        aliases: '张总',
        note: '产品负责人',
        enabled: true,
      },
    ])
  })

  it('uses an inline category form with validation instead of prompt', async () => {
    const user = userEvent.setup()
    const saving = deferred<{ id: string; name: string; scope: string; entryCount: number }>()
    adapterMocks.createCategoryAdapter.mockReturnValue(saving.promise)

    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByText('人名')
    await user.click(screen.getByRole('button', { name: /新建分类/i }))

    expect(screen.getByRole('heading', { name: '新建分类' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '保存分类' }))
    expect(screen.getByText('分类名称不能为空')).toBeInTheDocument()

    await user.type(screen.getByLabelText('分类名称'), '会议角色')
    await user.click(screen.getByRole('button', { name: '保存分类' }))

    expect(adapterMocks.createCategoryAdapter).toHaveBeenCalledWith('会议角色', 'global')
    expect(screen.getByText('保存中...')).toBeInTheDocument()

    saving.resolve({ id: 'cat-2', name: '会议角色', scope: 'global', entryCount: 0 })

    await screen.findByText('会议角色')
    expect(onNotice).toHaveBeenCalledWith('分类「会议角色」已创建')
  })

  it('guides the user when the selected scope has no categories', async () => {
    const user = userEvent.setup()

    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByText('人名')
    await user.selectOptions(screen.getByRole('combobox'), 'project')

    expect(await screen.findByText('当前范围暂无分类')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /新建词条/i })).toBeDisabled()
    expect(screen.getByText('词典条目目前用于管理和查阅，尚未接入自动 ASR 修正；历史转写不会被覆盖。')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '新建当前范围分类' }))
    expect(screen.getByRole('heading', { name: '新建分类' })).toBeInTheDocument()
  })

  it('retries a failed entry save from the inline editor', async () => {
    const user = userEvent.setup()
    adapterMocks.updateEntryAdapter
      .mockRejectedValueOnce(new Error('write failed'))
      .mockResolvedValueOnce(undefined)

    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByText('张伟')
    await user.click(screen.getByRole('button', { name: '张伟' }))
    await user.click(screen.getByRole('button', { name: '编辑词条' }))

    const termInput = screen.getByLabelText('标准词条')
    await user.clear(termInput)
    await user.type(termInput, '张玮')
    await user.click(screen.getByRole('button', { name: '保存词条' }))

    await screen.findByText('保存失败，请重试。')
    expect(adapterMocks.updateEntryAdapter).toHaveBeenCalledWith('ent-1', '张玮', 'zhang wei', '张总', '产品负责人')

    await user.click(screen.getByRole('button', { name: '重试保存' }))

    await waitFor(() => {
      expect(adapterMocks.updateEntryAdapter).toHaveBeenCalledTimes(2)
    })
    await waitFor(() => {
      expect(onNotice).toHaveBeenCalledWith('词条「张玮」已保存')
    })
  })

  it('clears stale entries while switching to a different category', async () => {
    const user = userEvent.setup()
    const nextCategoryEntries = deferred<Array<{
      id: string
      categoryId: string
      term: string
      pinyin: string
      aliases: string
      note: string
      enabled: boolean
    }>>()

    adapterMocks.loadCategories.mockResolvedValue([
      { id: 'cat-1', name: '人名', scope: 'global', entryCount: 1 },
      { id: 'cat-2', name: '术语', scope: 'global', entryCount: 1 },
    ])
    adapterMocks.loadEntries.mockImplementation((categoryId: string) => {
      if (categoryId === 'cat-1') {
        return Promise.resolve([
          {
            id: 'ent-1',
            categoryId: 'cat-1',
            term: '张伟',
            pinyin: 'zhang wei',
            aliases: '张总',
            note: '产品负责人',
            enabled: true,
          },
        ])
      }

      return nextCategoryEntries.promise
    })

    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByRole('button', { name: '张伟' })
    await user.click(screen.getByRole('button', { name: /术语/i }))

    expect(screen.queryByRole('button', { name: '张伟' })).not.toBeInTheDocument()
    expect(screen.getByText('词条加载中...')).toBeInTheDocument()

    nextCategoryEntries.resolve([
      {
        id: 'ent-2',
        categoryId: 'cat-2',
        term: '回写保护',
        pinyin: 'hui xie bao hu',
        aliases: '',
        note: '',
        enabled: true,
      },
    ])

    expect(await screen.findByRole('button', { name: '回写保护' })).toBeInTheDocument()
  })

  it('shows retry UI when categories fail to load', async () => {
    const user = userEvent.setup()

    adapterMocks.loadCategories
      .mockRejectedValueOnce(new Error('categories failed'))
      .mockResolvedValueOnce([{ id: 'cat-1', name: '人名', scope: 'global', entryCount: 1 }])

    render(<DictionaryView onNotice={onNotice} />)

    expect(await screen.findByText('分类加载失败，请重试。')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重试加载分类' }))
    expect(await screen.findByText('人名')).toBeInTheDocument()
  })

  it('shows retry UI when entries fail to load', async () => {
    const user = userEvent.setup()

    adapterMocks.loadEntries
      .mockRejectedValueOnce(new Error('entries failed'))
      .mockResolvedValueOnce([
        {
          id: 'ent-1',
          categoryId: 'cat-1',
          term: '张伟',
          pinyin: 'zhang wei',
          aliases: '张总',
          note: '产品负责人',
          enabled: true,
        },
      ])

    render(<DictionaryView onNotice={onNotice} />)

    expect(await screen.findByText('词条加载失败，请重试。')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重试加载词条' }))
    expect(await screen.findByRole('button', { name: '张伟' })).toBeInTheDocument()
  })

  it('keeps category counts in sync after entry create and delete', async () => {
    const user = userEvent.setup()

    adapterMocks.createEntryAdapter.mockResolvedValue({
      id: 'ent-2',
      categoryId: 'cat-1',
      term: '李娜',
      pinyin: 'li na',
      aliases: '',
      note: '',
      enabled: true,
    })
    adapterMocks.deleteEntryAdapter.mockResolvedValue(undefined)

    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByText('1 个词')
    await screen.findByRole('button', { name: '张伟' })
    await user.click(screen.getByRole('button', { name: '新建词条' }))
    await user.type(screen.getByLabelText('标准词条'), '李娜')
    await user.click(screen.getByRole('button', { name: '保存词条' }))

    await screen.findByText('2 个词')
    await user.click(screen.getByRole('button', { name: '李娜' }))
    await user.click(screen.getByRole('button', { name: '删除' }))
    await user.click(screen.getByRole('button', { name: '确认删除词条' }))

    await waitFor(() => {
      expect(screen.getByText('1 个词')).toBeInTheDocument()
    })
  })

  it('drills from categories to entries and detail while preserving search state', async () => {
    const user = userEvent.setup()

    render(<DictionaryView onNotice={onNotice} />)

    const view = await screen.findByRole('main')
    expect(view).toHaveClass('dictionary-view--mobile-categories')

    await user.click(screen.getByRole('button', { name: /人名/i }))
    expect(view).toHaveClass('dictionary-view--mobile-entries')

    const search = screen.getByRole('searchbox', { name: '搜索词条' })
    await user.type(search, '张')
    await user.click(await screen.findByRole('button', { name: '张伟' }))
    expect(view).toHaveClass('dictionary-view--mobile-detail')

    await user.click(screen.getByRole('button', { name: '返回词条' }))
    expect(view).toHaveClass('dictionary-view--mobile-entries')
    expect(search).toHaveValue('张')

    await user.click(screen.getByRole('button', { name: '返回分类' }))
    expect(view).toHaveClass('dictionary-view--mobile-categories')
  })

  it('marks an empty detail panel as compact and removes the repeated eyebrow', async () => {
    render(<DictionaryView onNotice={onNotice} />)

    await screen.findByText('人名')

    expect(screen.getByTestId('dictionary-detail')).toHaveClass('dictionary-view__detail--empty')
    expect(screen.getByTestId('dictionary-body')).toHaveClass('dictionary-view__body--detail-empty')
    expect(screen.queryByText('DICTIONARY')).not.toBeInTheDocument()
  })

  it('confirms category deletion inline with its name and irreversible impact', async () => {
    const user = userEvent.setup()
    const deletion = deferred<void>()
    adapterMocks.deleteCategoryAdapter.mockReturnValue(deletion.promise)
    const confirmSpy = vi.spyOn(window, 'confirm')

    render(<DictionaryView onNotice={onNotice} />)

    await user.click(await screen.findByRole('button', { name: '删除分类' }))

    expect(adapterMocks.deleteCategoryAdapter).not.toHaveBeenCalled()
    expect(confirmSpy).not.toHaveBeenCalled()
    expect(screen.getByRole('heading', { name: '删除分类「人名」？' })).toBeInTheDocument()
    expect(screen.getByText('该分类中的 1 个词条将一并永久删除，此操作无法撤销。')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '取消' }))
    expect(screen.getByRole('main')).toHaveClass('dictionary-view--mobile-categories')
    expect(adapterMocks.deleteCategoryAdapter).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: '删除分类' }))
    const confirmButton = screen.getByRole('button', { name: '确认删除分类' })
    await user.click(confirmButton)
    await user.click(confirmButton)

    expect(adapterMocks.deleteCategoryAdapter).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: '删除中...' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled()
    expect(within(screen.getByTestId('dictionary-detail')).getByRole('button', { name: '返回分类' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '删除分类' })).toBeDisabled()

    deletion.resolve()

    await waitFor(() => {
      expect(adapterMocks.deleteCategoryAdapter).toHaveBeenCalledWith('cat-1')
    })
  })

  it('confirms entry deletion inline with the entry name', async () => {
    const user = userEvent.setup()
    const deletion = deferred<void>()
    adapterMocks.deleteEntryAdapter.mockReturnValueOnce(deletion.promise).mockResolvedValueOnce(undefined)
    const confirmSpy = vi.spyOn(window, 'confirm')

    render(<DictionaryView onNotice={onNotice} />)

    await user.click(await screen.findByRole('button', { name: '张伟' }))
    await user.click(screen.getByRole('button', { name: '删除' }))

    expect(adapterMocks.deleteEntryAdapter).not.toHaveBeenCalled()
    expect(confirmSpy).not.toHaveBeenCalled()
    expect(screen.getByRole('heading', { name: '删除词条「张伟」？' })).toBeInTheDocument()

    const confirmButton = screen.getByRole('button', { name: '确认删除词条' })
    await user.click(confirmButton)
    await user.click(confirmButton)

    expect(adapterMocks.deleteEntryAdapter).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: '删除中...' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '返回词条' })).toBeDisabled()

    deletion.reject(new Error('delete failed'))

    expect(await screen.findByRole('button', { name: '确认删除词条' })).toBeEnabled()
    expect(onNotice).toHaveBeenCalledWith('词条删除失败，请重试')

    await user.click(screen.getByRole('button', { name: '确认删除词条' }))

    await waitFor(() => {
      expect(adapterMocks.deleteEntryAdapter).toHaveBeenCalledTimes(2)
    })
  })
})
