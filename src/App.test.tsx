import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('LifeSub navigation', () => {
  it('keeps audio import on the timeline instead of the sidebar', async () => {
    const user = userEvent.setup()
    render(<App />)
    const sidebar = within(screen.getByRole('navigation', { name: '主导航' }))

    expect(sidebar.getByRole('button', { name: '录音' })).toBeInTheDocument()
    expect(sidebar.getByRole('button', { name: '时间线' })).toBeInTheDocument()
    expect(sidebar.getByRole('button', { name: '词典' })).toBeInTheDocument()
    expect(sidebar.getByRole('button', { name: '设置' })).toBeInTheDocument()
    expect(sidebar.queryByRole('button', { name: '导入音频' })).not.toBeInTheDocument()

    await user.click(sidebar.getByRole('button', { name: '时间线' }))
    expect(screen.getByRole('button', { name: '导入音频' })).toBeInTheDocument()
  })

  it('defaults to live capture page', () => {
    render(<App />)
    expect(screen.getAllByText('浏览器演示').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByRole('button', { name: '开始演示' })).toBeInTheDocument()
    expect(screen.getByText('浏览器演示数据，不会录音或保存。')).toBeInTheDocument()
  })

  it('switches pages via sidebar', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '时间线' }))
    expect(screen.getAllByText('LifeSub 首版范围讨论').length).toBeGreaterThanOrEqual(1)
  })

  it('navigates to dictionary page', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '词典' }))
    expect(screen.getByText('常用词库')).toBeInTheDocument()
  })

  it('opens settings modal', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '设置' }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getAllByText('录音设置').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText('ASR 设置')).toBeInTheDocument()
  })

  it('closes settings modal on Esc', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '设置' }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('closes settings modal on overlay click', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '设置' }))
    const overlay = document.querySelector('.modal-overlay')
    expect(overlay).not.toBeNull()
    await user.click(overlay!)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('shows import audio notice', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '时间线' }))
    await user.click(screen.getByRole('button', { name: '导入音频' }))
    expect(screen.getByText(/浏览器演示模式仅支持示例数据/)).toBeInTheDocument()
  })
})
