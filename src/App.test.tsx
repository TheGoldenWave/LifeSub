import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('LifeSub navigation', () => {
  it('renders sidebar with all navigation items', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: '录音' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '时间线' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '导入音频' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '词典' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '设置' })).toBeInTheDocument()
  })

  it('defaults to live capture page', () => {
    render(<App />)
    expect(screen.getByText('准备就绪')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '开始记录' })).toBeInTheDocument()
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
    expect(screen.getByText('常用词库 · ASR 辅助修正')).toBeInTheDocument()
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
    const overlay = screen.getByRole('dialog')
    await user.click(overlay)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('shows import audio notice', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '导入音频' }))
    expect(screen.getByText(/导入音频功能将在时间线页面中可用/)).toBeInTheDocument()
  })
})