import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('LifeSub desktop experience', () => {
  it('moves a capture session through recording controls', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: '开始记录' }))
    expect(screen.getByText('正在记录')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '暂停' }))
    expect(screen.getByText('已暂停')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '继续' }))
    await user.click(screen.getByRole('button', { name: '停止' }))
    expect(screen.getByText('记录已封存')).toBeInTheDocument()
  })

  it('filters transcript evidence by keyword', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.type(screen.getByRole('searchbox', { name: '搜索转写' }), '证据链')

    expect(screen.getByText(/证据链必须保留原始转写/)).toBeInTheDocument()
    expect(screen.queryByText(/先确认首版范围/)).not.toBeInTheDocument()
  })

  it('creates a manual revision without hiding the original', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: '创建修订' }))
    await user.clear(screen.getByRole('textbox', { name: '修订文本' }))
    await user.type(screen.getByRole('textbox', { name: '修订文本' }), '首版重点是可靠、可定位的声音证据。')
    await user.click(screen.getByRole('button', { name: '保存修订' }))

    expect(screen.getAllByText('人工修订 · r2')).toHaveLength(2)
    expect(screen.getByRole('button', { name: '查看原始 r1' })).toBeInTheDocument()
  })

  it('shows local-first provider and privacy settings', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: '设置' }))

    expect(screen.getByRole('heading', { name: '设置' })).toBeInTheDocument()
    expect(screen.getByText('本地演示 ASR')).toBeInTheDocument()
    expect(screen.getByText('云端处理默认关闭')).toBeInTheDocument()
  })
})
