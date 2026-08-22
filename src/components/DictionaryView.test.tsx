import { render, screen, waitFor } from '@testing-library/react'
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
    vi.spyOn(window, 'prompt').mockReturnValue('')
    vi.spyOn(window, 'confirm').mockReturnValue(true)

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
    expect(screen.getByText('词典会影响未来任务中的 ASR 修正，不会回写或覆盖历史转写记录。')).toBeInTheDocument()
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

    await waitFor(() => {
      expect(screen.getByText('1 个词')).toBeInTheDocument()
    })
  })
})
