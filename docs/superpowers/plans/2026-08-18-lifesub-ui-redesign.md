# LifeSub UI 治理与功能调整 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 LifeSub V0.2 前端从 2 页面（时间线 + 占位设置）重构为 4 页面（录音 / 时间线 / 词典 / 设置弹窗），新增实时 ASR 流式转写、时间戳笔记、会话树形目录、24h 录音统计、分类词库管理，并统一设计 Token 体系。

**Architecture:** React 19 + TypeScript 纯组件驱动，无路由库。App shell 持有 `activePage` 状态和 `settingsOpen` 布尔值。Sidebar 导航切换页面，设置通过 Modal 弹窗承载。每个页面是独立组件，内部管理自己的局部状态。Domain 类型扩展覆盖新实体（Note、DictionaryEntry、Stats、RecordingSession）。样式沿用现有 design-tokens.css 暗色主题 + 等宽字体气质，不引入新的 CSS 框架。

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Lucide React, CSS Custom Properties (design tokens)

**Required references:**
- `docs/design/tokens/base.json`
- `.claude/rules/common/coding-style.md`
- `.claude/contexts/dev.md`

---

## File Map

### Domain & Data
- **Modify** `src/domain.ts` — 扩展 EvidenceRecord、新增 Note、DictionaryEntry、DictionaryCategory、StatsSnapshot、RecordingSession 类型
- **Modify** `src/data/demo.ts` — 新增 demo 词典数据、demo 统计数据、demo 笔记数据

### Services
- **Modify** `src/services/lifesub.ts` — 新增 ASR 设置读写、模型列表、Job 状态查询、词典 CRUD 的 typed invoke wrapper
- **Modify** `src/services/markdown.ts` — Markdown 导出格式支持笔记章节

### Components — Shared
- **Create** `src/components/Sidebar.tsx` — 侧边栏导航（4 项 + 设置按钮）
- **Create** `src/components/Modal.tsx` — 通用弹窗容器（遮罩 + Esc 关闭 + 动画）
- **Create** `src/components/TabBar.tsx` — 水平 Tab 切换条

### Pages — 录音
- **Create** `src/components/LiveCapture.tsx` — 录音首页：控制栏 + 实时转写 + 笔记面板
- **Create** `src/components/NotePanel.tsx` — 右侧笔记列表 + 内联编辑器
- **Create** `src/components/NoteEditor.tsx` — 笔记编辑表单（内容/时间戳/标签/关联段落）

### Pages — 时间线
- **Create** `src/components/SessionTree.tsx` — 树状会话目录（会议 → 段落 → 笔记子节点）
- **Create** `src/components/StatsBar.tsx` — 底部 24h 块状统计条 + 本周/本月/累计
- **Modify** `src/components/TranscriptView.tsx` — 微调以适配新布局（去除 header 中重复的搜索框等）

### Pages — 词典
- **Create** `src/components/DictionaryView.tsx` — 词典页面：分类列表 + 词条列表 + 词条编辑
- **Create** `src/components/DictionaryEntry.tsx` — 单个词条组件（启用/停用/编辑/删除）

### Pages — 设置弹窗
- **Create** `src/components/SettingsModal.tsx` — 设置弹窗容器：左侧 Tab 导航 + 右侧内容区
- **Create** `src/components/RecordingSettings.tsx` — 录音设置 Tab：捕获模式 + IM 检测 + 格式 + 存储
- **Create** `src/components/AsrSettings.tsx` — ASR 设置 Tab：Provider + 语言/VAD/线程 + 专属选项 + 声纹库
- **Create** `src/components/ModelManager.tsx` — 模型 Tab：已安装/可安装列表 + 下载进度
- **Create** `src/components/AboutTab.tsx` — 关于 Tab：版本 + 运行时 + 许可

### App Shell
- **Modify** `src/App.tsx` — 重构为 4 页面切换 + 设置弹窗状态管理
- **Modify** `src/App.test.tsx` — 更新现有测试，新增页面切换、设置弹窗、词典操作测试
- **Modify** `src/styles.css` — 新增页面级样式、弹窗样式、树组件样式、统计条样式、词典样式

### Tests
- **Create** `src/components/LiveCapture.test.tsx` — 录音页面测试
- **Create** `src/components/SessionTree.test.tsx` — 树目录测试
- **Create** `src/components/StatsBar.test.tsx` — 统计条测试
- **Create** `src/components/DictionaryView.test.tsx` — 词典页面测试
- **Create** `src/components/SettingsModal.test.tsx` — 设置弹窗测试

---

## Chunk 1: Domain Types & Data Model Extensions

### Task 1: Extend domain types

**Files:**
- Modify: `src/domain.ts`

- [ ] **Step 1: Add new domain types to `src/domain.ts`**

```typescript
// 现有类型保持不变，追加以下类型

/** 笔记标签 */
export type NoteTag = '待办' | '备忘' | '问题' | '决定' | string

/** 录音过程中添加的笔记 */
export interface CaptureNote {
  id: string
  /** 笔记内容 */
  content: string
  /** 关联的时间戳 (ms) */
  timestampMs: number
  /** 标签分类 */
  tag: NoteTag
  /** 关联的 ASR 段落 ID（可选） */
  segmentId: string | null
  /** 创建时间 */
  createdAt: string
}

/** 录音会话（实时录音页面的状态） */
export interface RecordingSession {
  id: string
  title: string
  startedAt: string
  state: CaptureState
  /** 当前捕获模式 */
  captureMode: CaptureMode
  /** 实时 ASR 段落（流式追加） */
  segments: LiveSegment[]
  /** 笔记列表 */
  notes: CaptureNote[]
}

/** 捕获模式 */
export type CaptureMode = 'smart' | 'mic-only' | 'system-only'

/** 声纹库中的注册说话人 */
export interface Voiceprint {
  /** 唯一 ID，与 Speaker.id 对应 */
  id: string
  /** 显示名称（如 "张伟"） */
  name: string
  /** 声纹特征向量存储路径（本地文件） */
  embeddingPath: string
  /** 关联的词典词条 ID（可选，如 "ent-1" 对应词典中的"张伟"） */
  dictionaryEntryId: string | null
  /** 声纹样本数量 */
  sampleCount: number
  /** 最后更新时间 */
  updatedAt: string
}

/** 说话人标识 */
export interface Speaker {
  /** 唯一 ID（如 "spk-1"） */
  id: string
  /** 显示名称（如 "张伟"、"未知说话人 1"） */
  label: string
  /**
   * 识别来源：
   * - voiceprint  — 声纹匹配成功，自动标注
   * - dictionary  — 词典匹配（人名命中），但无声纹
   * - manual      — 用户手动标注
   * - unknown     — 未识别，等待用户标注
   */
  source: 'voiceprint' | 'dictionary' | 'manual' | 'unknown'
  /** 关联的声纹 ID（当 source === 'voiceprint' 时） */
  voiceprintId: string | null
}

/** 实时流式段落 */
export interface LiveSegment {
  id: string
  startMs: number
  /** 说话人（按人名标注，非声道） */
  speaker: Speaker
  /** 流式文本（可能不完整，末尾有 ▌光标） */
  text: string
  /** 是否已完成（VAD 判定说话结束） */
  completed: boolean
}

/** 词典分类 */
export interface DictionaryCategory {
  id: string
  name: string
  /** 分类来源：global 全局 / project:{id} 项目补充 */
  scope: 'global' | string
  entryCount: number
}

/** 词典词条 */
export interface DictionaryEntry {
  id: string
  /** 所属分类 ID */
  categoryId: string
  /** 正确写法 */
  term: string
  /** 拼音 */
  pinyin: string
  /** 别名（分号分隔，ASR 会将这些也映射到 term） */
  aliases: string
  /** 备注 */
  note: string
  /** 是否启用 */
  enabled: boolean
}

/** 24 小时录音统计快照 */
export interface StatsSnapshot {
  /** 24 小时槽位，每个表示该小时是否有录音 */
  hourlySlots: Array<{
    hour: number          // 0-23
    minutes: number       // 该小时录音总分钟数
    sessionId: string | null  // 关联的会话 ID
    title: string | null      // 关联的会话标题
  }>
  /** 本周统计 */
  weekSessions: number
  weekMinutes: number
  /** 本月统计 */
  monthSessions: number
  monthMinutes: number
  /** 累计统计 */
  totalSessions: number
  totalMinutes: number
}
```

- [ ] **Step 2: Extend `EvidenceRecord` to include notes**

```typescript
// 在现有 EvidenceRecord 接口中追加字段：
export interface EvidenceRecord {
  // ... 现有字段保持不变 ...
  /** 录音过程中添加的笔记 */
  notes: CaptureNote[]
}
```

- [ ] **Step 3: Add demo data for new types**

Modify `src/data/demo.ts`:

```typescript
// 追加到现有 demoRecords 的每个 record 中：
// records[0].notes = demoNotes
// records[1].notes = []

export const demoNotes: CaptureNote[] = [
  {
    id: 'note-001',
    content: '确认 ASR Provider 切换时机',
    timestampMs: 4_000,
    tag: '待办',
    segmentId: 'seg-001',
    createdAt: '2026-08-18T16:20:00Z',
  },
  {
    id: 'note-002',
    content: '证据链包含原始音频+转写+修订记录',
    timestampMs: 20_000,
    tag: '备忘',
    segmentId: 'seg-002',
    createdAt: '2026-08-18T16:22:00Z',
  },
  {
    id: 'note-003',
    content: '落实搜索结果时间戳关联',
    timestampMs: 32_000,
    tag: '待办',
    segmentId: 'seg-003',
    createdAt: '2026-08-18T16:24:00Z',
  },
]

export const demoCategories: DictionaryCategory[] = [
  { id: 'cat-1', name: '人名', scope: 'global', entryCount: 8 },
  { id: 'cat-2', name: '地名', scope: 'global', entryCount: 12 },
  { id: 'cat-3', name: '专业术语', scope: 'global', entryCount: 25 },
  { id: 'cat-4', name: '项目名', scope: 'global', entryCount: 6 },
  { id: 'cat-5', name: '品牌名', scope: 'global', entryCount: 3 },
  { id: 'cat-proj-1', name: 'LifeSub 术语', scope: 'project:lifesub', entryCount: 4 },
]

export const demoEntries: DictionaryEntry[] = [
  { id: 'ent-1', categoryId: 'cat-1', term: '张伟', pinyin: 'zhāng wěi', aliases: '张总;伟哥', note: '产品负责人', enabled: true },
  { id: 'ent-2', categoryId: 'cat-1', term: '李娜', pinyin: 'lǐ nà', aliases: '', note: '', enabled: true },
  { id: 'ent-3', categoryId: 'cat-1', term: '刘洋', pinyin: 'liú yáng', aliases: '', note: '', enabled: false },
]

export const demoVoiceprints: Voiceprint[] = [
  { id: 'vp-1', name: '张伟', embeddingPath: '~/.lifesub/voiceprints/vp-1.bin', dictionaryEntryId: 'ent-1', sampleCount: 12, updatedAt: '2026-08-18T16:00:00Z' },
  { id: 'vp-2', name: '我', embeddingPath: '~/.lifesub/voiceprints/vp-2.bin', dictionaryEntryId: null, sampleCount: 8, updatedAt: '2026-08-17T10:00:00Z' },
]

export const demoStats: StatsSnapshot = {
  hourlySlots: Array.from({ length: 24 }, (_, i) => ({
    hour: i,
    minutes: i === 15 ? 22 : i === 16 ? 38 : 0,
    sessionId: i === 15 ? 'rec-20260814-002' : i === 16 ? 'rec-20260815-001' : null,
    title: i === 15 ? '架构边界复盘' : i === 16 ? 'LifeSub 首版范围讨论' : null,
  })),
  weekSessions: 3,
  weekMinutes: 62,
  monthSessions: 8,
  monthMinutes: 180,
  totalSessions: 42,
  totalMinutes: 1560,
}
```

- [ ] **Step 4: Run type check**

```bash
cd src && npx tsc --noEmit
```

- [ ] **Step 5: Commit**

```bash
git add src/domain.ts src/data/demo.ts
git commit -m "feat: extend domain types for UI redesign (Note, Dictionary, Stats, LiveSegment)"
```

---

## Chunk 2: Shared Components (Sidebar, Modal, TabBar)

### Task 2: Create Sidebar component

**Files:**
- Create: `src/components/Sidebar.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write Sidebar component**

```tsx
import { Mic, Archive, BookOpen, Settings, AudioLines } from 'lucide-react'

export type PageId = 'live' | 'timeline' | 'dictionary'

interface SidebarProps {
  activePage: PageId
  onNavigate: (page: PageId) => void
  onImportAudio: () => void
  onOpenSettings: () => void
}

export function Sidebar({ activePage, onNavigate, onImportAudio, onOpenSettings }: SidebarProps) {
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
        <button className="nav-item nav-item--action" onClick={onImportAudio}>
          导入音频
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
```

- [ ] **Step 2: Add Sidebar styles to `src/styles.css`**

```css
/* 追加到 sidebar 区域 */
.nav-item--action {
  border: 1px dashed var(--colors-brand-borderStrong);
  margin: var(--spacing-2) 0;
  color: var(--colors-brand-textSecondary);
}
.nav-item--action:hover {
  border-style: solid;
  color: var(--colors-brand-textPrimary);
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/Sidebar.tsx src/styles.css
git commit -m "feat: add Sidebar with 4-page navigation"
```

### Task 3: Create Modal component

**Files:**
- Create: `src/components/Modal.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write Modal component**

```tsx
import { useEffect, useRef, type ReactNode } from 'react'
import { X } from 'lucide-react'

interface ModalProps {
  open: boolean
  onClose: () => void
  title: string
  children: ReactNode
}

export function Modal({ open, onClose, title, children }: ModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [open, onClose])

  if (!open) return null

  return (
    <div
      className="modal-overlay"
      ref={overlayRef}
      onClick={(e) => { if (e.target === overlayRef.current) onClose() }}
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div className="modal-container">
        <header className="modal-header">
          <h2>{title}</h2>
          <button className="icon-button" aria-label="关闭设置" onClick={onClose}>
            <X size={18} />
          </button>
        </header>
        <div className="modal-body">
          {children}
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Add Modal styles to `src/styles.css`**

```css
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--zIndex-overlay);
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.6);
}
.modal-container {
  width: min(900px, 95vw);
  max-height: 85vh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  border: 1px solid var(--colors-brand-borderStrong);
  border-radius: 0;
  background: var(--colors-brand-surface);
  overflow: hidden;
}
.modal-header {
  padding: var(--spacing-4) var(--spacing-6);
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--colors-brand-border);
}
.modal-header h2 {
  margin: 0;
  font-family: var(--typography-fontFamily-mono);
  font-size: var(--typography-fontSize-sm);
  font-weight: var(--typography-fontWeight-medium);
  text-transform: uppercase;
  letter-spacing: 0.1em;
}
.modal-body {
  overflow: auto;
  display: grid;
  grid-template-columns: 180px minmax(0, 1fr);
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/Modal.tsx src/styles.css
git commit -m "feat: add Modal component with overlay and Esc-to-close"
```

### Task 4: Create TabBar component

**Files:**
- Create: `src/components/TabBar.tsx`

- [ ] **Step 1: Write TabBar component**

```tsx
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
```

- [ ] **Step 2: Add TabBar styles to `src/styles.css`**

```css
.tab-bar {
  display: flex;
  border-bottom: 1px solid var(--colors-brand-border);
}
.tab-bar__tab {
  padding: var(--spacing-3) var(--spacing-4);
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--colors-brand-textMuted);
  background: transparent;
  font-family: var(--typography-fontFamily-mono);
  font-size: var(--typography-fontSize-xs);
  letter-spacing: 0.06em;
  cursor: pointer;
}
.tab-bar__tab:hover {
  color: var(--colors-brand-textSecondary);
  background: var(--colors-brand-surfaceSubtle);
}
.tab-bar__tab--active {
  color: var(--colors-brand-textPrimary);
  border-bottom-color: var(--colors-brand-textPrimary);
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/TabBar.tsx src/styles.css
git commit -m "feat: add TabBar component"
```

---

## Chunk 3: App Shell Refactor

### Task 5: Refactor App.tsx to 4-page shell

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Rewrite App.tsx**

```tsx
import { useState } from 'react'
import { Sidebar, type PageId } from './components/Sidebar'
import { LiveCapture } from './components/LiveCapture'
import { TimelineView } from './components/TimelineView'
import { DictionaryView } from './components/DictionaryView'
import { SettingsModal } from './components/SettingsModal'
import { demoRecords, demoNotes, demoCategories, demoEntries, demoStats } from './data/demo'
import type { EvidenceRecord, CaptureNote, DictionaryCategory, DictionaryEntry, StatsSnapshot } from './domain'

export default function App() {
  const [activePage, setActivePage] = useState<PageId>('live')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [records, setRecords] = useState<EvidenceRecord[]>(
    demoRecords.map((r) => ({ ...r, notes: r.id === demoRecords[0].id ? demoNotes : [] }))
  )
  const [notice, setNotice] = useState('')

  const handleImportAudio = () => {
    // TODO: Task 5 migration — wire up real import flow
    setNotice('导入音频功能将在时间线页面中可用。')
  }

  return (
    <div className="app-shell">
      <Sidebar
        activePage={activePage}
        onNavigate={setActivePage}
        onImportAudio={handleImportAudio}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <section className="workspace">
        {notice && (
          <div className="notice" role="status">
            {notice}
            <button aria-label="关闭提示" onClick={() => setNotice('')}>×</button>
          </div>
        )}
        {activePage === 'live' && <LiveCapture onNotice={setNotice} />}
        {activePage === 'timeline' && (
          <TimelineView
            records={records}
            onRecordsChange={setRecords}
            onNotice={setNotice}
          />
        )}
        {activePage === 'dictionary' && (
          <DictionaryView
            categories={demoCategories}
            entries={demoEntries}
            onNotice={setNotice}
          />
        )}
      </section>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  )
}
```

- [ ] **Step 2: Create placeholder page components**

Create stub files so the build passes while we implement each page:

```tsx
// src/components/LiveCapture.tsx (stub)
export function LiveCapture({ onNotice }: { onNotice: (msg: string) => void }) {
  return <main className="page-placeholder"><h1>Live Capture</h1><p>实时录音与转写</p></main>
}

// src/components/TimelineView.tsx (stub)
export function TimelineView({ records, onRecordsChange, onNotice }: any) {
  return <main className="page-placeholder"><h1>Timeline</h1><p>会话与证据</p></main>
}

// src/components/DictionaryView.tsx (stub)
export function DictionaryView({ categories, entries, onNotice }: any) {
  return <main className="page-placeholder"><h1>Dictionary</h1><p>常用词库</p></main>
}

// src/components/SettingsModal.tsx (stub)
export function SettingsModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  if (!open) return null
  return <div className="modal-overlay" onClick={onClose}><div className="modal-container"><h2>设置</h2></div></div>
}
```

Add placeholder style:

```css
.page-placeholder {
  min-height: 0;
  padding: var(--spacing-16);
  display: grid;
  place-items: center;
  color: var(--colors-brand-textMuted);
  border: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-surface);
}
```

- [ ] **Step 3: Update App.test.tsx — migrate existing tests**

Update tests to match new page structure:

```tsx
// 替换现有测试中的 import
import App from './App'

describe('LifeSub navigation', () => {
  it('renders sidebar with navigation items', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: '录音' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '时间线' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '词典' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '设置' })).toBeInTheDocument()
  })

  it('defaults to live capture page', () => {
    render(<App />)
    expect(screen.getByText('实时录音与转写')).toBeInTheDocument()
  })

  it('switches pages via sidebar', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '时间线' }))
    expect(screen.getByText('会话与证据')).toBeInTheDocument()
  })

  it('opens settings modal', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '设置' }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('closes settings modal on Esc', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: '设置' }))
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})
```

- [ ] **Step 4: Run tests**

```bash
npm test -- --run src/App.test.tsx
```

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/App.test.tsx src/components/LiveCapture.tsx src/components/TimelineView.tsx src/components/DictionaryView.tsx src/components/SettingsModal.tsx src/styles.css
git commit -m "feat: refactor App shell to 4-page navigation with sidebar"
```

---

## Chunk 4: 录音页面 (LiveCapture)

### Task 6: Implement LiveCapture page

**Files:**
- Modify: `src/components/LiveCapture.tsx`
- Create: `src/components/NotePanel.tsx`
- Create: `src/components/NoteEditor.tsx`
- Create: `src/components/LiveCapture.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write LiveCapture component**

> **说话人标注策略**：说话人按**人名**标注，不按声道。双声道分轨仅用于提升 ASR 准确率（左右声道独立音频流），转写结果中的说话人标注通过以下流程确定：
>
> ```
> 双声道音频 → 说话人分离 (Diarization) → 声纹匹配 → 词典辅助 → 人工标注
>                  ├─ 切分出 spk-1/2/3     ├─ 命中→自动命名   ├─ 词典人名命中  ├─ 用户点击
>                  └─ 匿名 ID              └─ 未命中→"未知说话人 N"    └─ 提示但未确认   └─ 保存声纹
> ```
>
> Speaker.source 四种来源：
> - `voiceprint`（绿色实线）—— 声纹匹配成功，自动标注为"张伟"
> - `dictionary`（绿色虚线）—— 词典人名命中但无声纹，提示"可能是李娜？"，待确认
> - `manual`（蓝色）—— 用户手动标注，如"我"、自定义名称
> - `unknown`（黄色虚线）—— 未识别，显示"未知说话人 1"，点击可重命名并保存声纹
>
> 重命名未知说话人时，自动将其声纹样本保存到声纹库，后续会话自动识别。

```tsx
import { useState } from 'react'
import { CirclePause, Square, Plus, Copy } from 'lucide-react'
import { NotePanel } from './NotePanel'
import type { CaptureMode, CaptureState, LiveSegment, CaptureNote } from '../domain'

interface LiveCaptureProps {
  onNotice: (msg: string) => void
}

const DEMO_SEGMENTS: LiveSegment[] = [
  { id: 'ls-1', startMs: 12000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '我们今天先确认首版范围，重点是把基础闭环真正跑起来。', completed: true },
  { id: 'ls-2', startMs: 18000, speaker: { id: 'spk-2', label: '我', source: 'manual', voiceprintId: null }, text: '好的，我记一下。证据链要保证每次修改都能追溯。', completed: true },
  { id: 'ls-3', startMs: 25000, speaker: { id: 'spk-1', label: '张伟', source: 'voiceprint', voiceprintId: 'vp-1' }, text: '对，而且要保证搜索结果能回到准确的音频时间范围。', completed: true },
  { id: 'ls-4', startMs: 32000, speaker: { id: 'spk-3', label: '可能是李娜？', source: 'dictionary', voiceprintId: null }, text: '还有一个点，关于数据目录的权限控制...', completed: true },
  { id: 'ls-5', startMs: 45000, speaker: { id: 'spk-4', label: '未知说话人 1', source: 'unknown', voiceprintId: null }, text: '这个方案我觉得可以...', completed: false },
]

export function LiveCapture({ onNotice }: LiveCaptureProps) {
  const [captureState, setCaptureState] = useState<CaptureState>('idle')
  const [captureMode, setCaptureMode] = useState<CaptureMode>('smart')
  const [segments, setSegments] = useState<LiveSegment[]>([])
  const [notes, setNotes] = useState<CaptureNote[]>([])
  const [showDemo, setShowDemo] = useState(false)

  const formatTime = (ms: number) => {
    const s = Math.floor(ms / 1000)
    return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
  }

  const startCapture = () => {
    setCaptureState('recording')
    setSegments([])
    setNotes([])
    // Demo: simulate streaming after 1s
    setTimeout(() => {
      setSegments(DEMO_SEGMENTS)
      setShowDemo(true)
    }, 1000)
  }

  const stopCapture = () => {
    setCaptureState('stopped')
    onNotice('录音已保存，可在时间线页面查看。')
  }

  const addNote = (note: CaptureNote) => {
    setNotes((prev) => [...prev, note].sort((a, b) => a.timestampMs - b.timestampMs))
  }

  const deleteNote = (id: string) => {
    setNotes((prev) => prev.filter((n) => n.id !== id))
  }

  const copyAll = async () => {
    const text = segments.map((s) => `[${formatTime(s.startMs)}] ${s.speaker} ${s.text}`).join('\n\n')
    await navigator.clipboard.writeText(text)
    onNotice('转写全文已复制。')
  }

  return (
    <main className="live-capture">
      <header className="live-capture__bar">
        <div className="live-capture__status">
          <span className={`recorder__pulse recorder__pulse--${captureState}`} aria-hidden="true" />
          <div>
            <strong>
              {captureState === 'idle' ? '准备就绪' : captureState === 'recording' ? '正在记录' : '记录已封存'}
            </strong>
            <small>{captureMode === 'smart' ? '智能路由 · 单声道 · 未检测到通话' : '仅麦克风'}</small>
          </div>
        </div>
        <div className="live-capture__asr">
          SenseVoice · 中文 · ITN 开启
        </div>
        <div className="live-capture__actions">
          {captureState === 'idle' && (
            <button className="button button--primary" onClick={startCapture}>
              <span className="recorder__pulse recorder__pulse--recording" /> 开始记录
            </button>
          )}
          {captureState === 'recording' && (
            <>
              <button className="button" onClick={() => setCaptureState('paused')}>
                <CirclePause size={17} />暂停
              </button>
              <button className="button" onClick={addNote.bind(null, {
                id: `note-${Date.now()}`,
                content: '',
                timestampMs: 0,
                tag: '备忘',
                segmentId: null,
                createdAt: new Date().toISOString(),
              })}>
                <Plus size={17} />笔记
              </button>
              <button className="button button--danger" onClick={stopCapture}>
                <Square size={15} />停止
              </button>
            </>
          )}
        </div>
      </header>

      <div className="live-capture__body">
        <section className="live-capture__transcript">
          <div className="live-capture__transcript-header">
            <span className="eyebrow">实时转写</span>
            {segments.length > 0 && (
              <button className="text-button" onClick={copyAll}>
                <Copy size={14} />复制全部
              </button>
            )}
          </div>

          {segments.length === 0 && !showDemo && captureState === 'idle' && (
            <div className="empty-state">
              <strong>💡 尚未开始录音</strong>
              <p>点击「开始记录」或按 ⌘R 启动实时转写</p>
            </div>
          )}
          {segments.length === 0 && captureState === 'recording' && (
            <div className="empty-state">
              <strong>🎤 正在监听...</strong>
              <p>检测到语音后将自动开始转写</p>
            </div>
          )}

          <div className="live-capture__segments">
            {segments.map((seg) => (
              <article key={seg.id} className="live-segment">
                <div className="live-segment__header">
                  <span
                    className={`live-segment__speaker live-segment__speaker--${seg.speaker.source}`}
                    title={seg.speaker.source === 'unknown' ? '点击重命名说话人' : seg.speaker.source === 'dictionary' ? '来自词典' : '手动标注'}
                  >
                    {seg.speaker.label}
                  </span>
                  <span className="live-segment__time">{formatTime(seg.startMs)}</span>
                  {!seg.completed && <span className="live-segment__cursor">▌</span>}
                </div>
                <p className="live-segment__text">{seg.text}</p>
              </article>
            ))}
          </div>
        </section>

        <NotePanel
          notes={notes}
          onAdd={addNote}
          onDelete={deleteNote}
          segments={segments}
        />
      </div>
    </main>
  )
}
```

- [ ] **Step 2: Write NotePanel and NoteEditor**

```tsx
// src/components/NotePanel.tsx
import { useState } from 'react'
import { Plus, X } from 'lucide-react'
import { NoteEditor } from './NoteEditor'
import type { CaptureNote, LiveSegment } from '../domain'

interface NotePanelProps {
  notes: CaptureNote[]
  onAdd: (note: CaptureNote) => void
  onDelete: (id: string) => void
  segments: LiveSegment[]
}

function formatTimestamp(ms: number) {
  const s = Math.floor(ms / 1000)
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
}

export function NotePanel({ notes, onAdd, onDelete, segments }: NotePanelProps) {
  const [editing, setEditing] = useState(false)

  return (
    <aside className="note-panel">
      <header className="note-panel__header">
        <span className="eyebrow">笔记 ({notes.length})</span>
        <button className="text-button" onClick={() => setEditing(true)}>
          <Plus size={14} />新笔记
        </button>
      </header>

      {editing && (
        <NoteEditor
          onSave={(note) => { onAdd(note); setEditing(false) }}
          onCancel={() => setEditing(false)}
          segments={segments}
        />
      )}

      <div className="note-panel__list">
        {notes.map((note) => (
          <article key={note.id} className={`note-card note-card--${note.tag}`}>
            <div className="note-card__header">
              <span className="note-card__time">{formatTimestamp(note.timestampMs)}</span>
              <span className="note-card__tag">{note.tag}</span>
              <button className="text-button" onClick={() => onDelete(note.id)} aria-label="删除笔记">
                <X size={12} />
              </button>
            </div>
            <p className="note-card__content">{note.content}</p>
          </article>
        ))}
        {notes.length === 0 && !editing && (
          <p className="note-panel__empty">暂无笔记，点击「新笔记」添加。</p>
        )}
      </div>
    </aside>
  )
}

// src/components/NoteEditor.tsx
import { useState } from 'react'
import type { CaptureNote, NoteTag, LiveSegment } from '../domain'

interface NoteEditorProps {
  onSave: (note: CaptureNote) => void
  onCancel: () => void
  segments: LiveSegment[]
}

const TAGS: NoteTag[] = ['待办', '备忘', '问题', '决定']

export function NoteEditor({ onSave, onCancel, segments }: NoteEditorProps) {
  const [content, setContent] = useState('')
  const [tag, setTag] = useState<NoteTag>('备忘')

  const handleSave = () => {
    if (!content.trim()) return
    onSave({
      id: `note-${Date.now()}`,
      content: content.trim(),
      timestampMs: Date.now() % 3600000, // demo: relative ms
      tag,
      segmentId: null,
      createdAt: new Date().toISOString(),
    })
  }

  return (
    <div className="note-editor">
      <textarea
        className="note-editor__input"
        placeholder="输入笔记内容..."
        value={content}
        onChange={(e) => setContent(e.target.value)}
        rows={3}
      />
      <div className="note-editor__meta">
        <select
          className="note-editor__tag-select"
          value={tag}
          onChange={(e) => setTag(e.target.value as NoteTag)}
        >
          {TAGS.map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>
      <div className="note-editor__actions">
        <button className="text-button" onClick={onCancel}>取消</button>
        <button className="button button--primary" onClick={handleSave}>保存</button>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Add LiveCapture styles**

```css
.live-capture {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-surface);
}
.live-capture__bar {
  padding: var(--spacing-4) var(--spacing-6);
  display: flex;
  align-items: center;
  gap: var(--spacing-6);
  border-bottom: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.live-capture__status { display: flex; align-items: center; gap: var(--spacing-3); min-width: 200px; }
.live-capture__status strong { display: block; font-family: var(--typography-fontFamily-mono); font-size: var(--typography-fontSize-sm); }
.live-capture__status small { color: var(--colors-brand-textMuted); font-family: var(--typography-fontFamily-mono); font-size: 11px; }
.live-capture__asr {
  padding: var(--spacing-1) var(--spacing-3);
  border: 1px solid var(--colors-brand-border);
  color: var(--colors-brand-textMuted);
  font-family: var(--typography-fontFamily-mono);
  font-size: 11px;
}
.live-capture__actions { margin-left: auto; display: flex; gap: var(--spacing-2); }
.live-capture__body {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  overflow: hidden;
}
.live-capture__transcript { overflow: auto; padding: var(--spacing-6); }
.live-capture__transcript-header {
  display: flex; justify-content: space-between; align-items: center;
  margin-bottom: var(--spacing-6); padding-bottom: var(--spacing-3);
  border-bottom: 1px solid var(--colors-brand-border);
}
.live-segment { padding: var(--spacing-4) 0; border-bottom: 1px solid var(--colors-brand-border); }
.live-segment__header { display: flex; align-items: center; gap: var(--spacing-2); margin-bottom: var(--spacing-2); }
.live-segment__speaker { font-family: var(--typography-fontFamily-mono); font-size: var(--typography-fontSize-sm); font-weight: var(--typography-fontWeight-medium); }
.live-segment__speaker--voiceprint { color: var(--colors-brand-available); }
.live-segment__speaker--dictionary { color: var(--colors-brand-available); border-bottom: 1px dashed var(--colors-brand-available); cursor: pointer; }
.live-segment__speaker--manual { color: var(--colors-brand-focus); }
.live-segment__speaker--unknown { color: var(--colors-brand-paused); cursor: pointer; border-bottom: 1px dashed var(--colors-brand-paused); }
.live-segment__time { color: var(--colors-brand-textMuted); font-family: var(--typography-fontFamily-mono); font-size: 11px; }
.live-segment__cursor { color: var(--colors-brand-focus); animation: blink 1s step-end infinite; }
@keyframes blink { 50% { opacity: 0; } }
.live-segment__text { max-width: 70ch; margin: 0; font-size: var(--typography-fontSize-lg); line-height: var(--typography-lineHeight-loose); }

/* Note Panel */
.note-panel {
  overflow: auto; padding: var(--spacing-4);
  border-left: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.note-panel__header { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--spacing-4); }
.note-panel__list { display: grid; gap: var(--spacing-2); }
.note-panel__empty { color: var(--colors-brand-textMuted); font-size: var(--typography-fontSize-xs); text-align: center; padding: var(--spacing-8); }
.note-card { padding: var(--spacing-3); border-left: 2px solid var(--colors-brand-border); background: var(--colors-brand-surface); }
.note-card--待办 { border-left-color: var(--colors-brand-paused); }
.note-card--备忘 { border-left-color: var(--colors-brand-focus); }
.note-card--问题 { border-left-color: var(--colors-brand-recording); }
.note-card--决定 { border-left-color: var(--colors-brand-available); }
.note-card__header { display: flex; align-items: center; gap: var(--spacing-2); margin-bottom: var(--spacing-1); }
.note-card__time { font-family: var(--typography-fontFamily-mono); font-size: 11px; color: var(--colors-brand-textMuted); }
.note-card__tag { padding: 1px var(--spacing-1); font-family: var(--typography-fontFamily-mono); font-size: 10px; border: 1px solid var(--colors-brand-border); }
.note-card__content { margin: 0; font-size: var(--typography-fontSize-sm); color: var(--colors-brand-textSecondary); }

/* Note Editor */
.note-editor { margin-bottom: var(--spacing-4); padding: var(--spacing-3); border: 1px solid var(--colors-brand-borderStrong); background: var(--colors-brand-surface); }
.note-editor__input { width: 100%; padding: var(--spacing-2); border: 1px solid var(--colors-brand-border); border-radius: var(--borderRadius-sm); color: var(--colors-brand-textPrimary); background: var(--colors-brand-canvas); font-size: var(--typography-fontSize-sm); resize: vertical; }
.note-editor__meta { margin-top: var(--spacing-2); }
.note-editor__tag-select { padding: var(--spacing-1) var(--spacing-2); border: 1px solid var(--colors-brand-border); color: var(--colors-brand-textPrimary); background: var(--colors-brand-canvas); font-size: var(--typography-fontSize-xs); }
.note-editor__actions { margin-top: var(--spacing-2); display: flex; justify-content: flex-end; gap: var(--spacing-2); }

/* Pulse states */
.recorder__pulse--idle { background: var(--colors-brand-textMuted); }
.recorder__pulse--recording { background: var(--colors-brand-recording); }
.recorder__pulse--paused { background: var(--colors-brand-paused); }
.recorder__pulse--stopped { background: var(--colors-brand-available); }
```

- [ ] **Step 4: Write LiveCapture tests**

```tsx
// src/components/LiveCapture.test.tsx
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { LiveCapture } from './LiveCapture'

describe('LiveCapture', () => {
  it('renders idle state', () => {
    render(<LiveCapture onNotice={() => {}} />)
    expect(screen.getByText('准备就绪')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '开始记录' })).toBeInTheDocument()
  })

  it('shows empty prompt when idle', () => {
    render(<LiveCapture onNotice={() => {}} />)
    expect(screen.getByText(/尚未开始录音/)).toBeInTheDocument()
  })

  it('starts recording and shows demo segments', async () => {
    const user = userEvent.setup()
    render(<LiveCapture onNotice={() => {}} />)
    await user.click(screen.getByRole('button', { name: '开始记录' }))
    expect(screen.getByText('正在记录')).toBeInTheDocument()
  })
})
```

- [ ] **Step 5: Run tests**

```bash
npm test -- --run src/components/LiveCapture.test.tsx
```

- [ ] **Step 6: Commit**

```bash
git add src/components/LiveCapture.tsx src/components/NotePanel.tsx src/components/NoteEditor.tsx src/components/LiveCapture.test.tsx src/styles.css
git commit -m "feat: implement LiveCapture page with streaming ASR and note panel"
```

---

## Chunk 5: 时间线页面 (TimelineView)

### Task 7: Implement SessionTree component

**Files:**
- Create: `src/components/SessionTree.tsx`
- Create: `src/components/SessionTree.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write SessionTree component**

```tsx
import { useState } from 'react'
import { ChevronRight, ChevronDown, Mic, Volume2, FileText, Clock } from 'lucide-react'
import type { EvidenceRecord, TranscriptSegment, CaptureNote } from '../domain'

interface SessionTreeProps {
  records: EvidenceRecord[]
  selectedId: string
  onSelect: (id: string) => void
  query: string
}

function formatTime(ms: number) {
  const s = Math.floor(ms / 1000)
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`
}

function SegmentIcon({ source }: { source: TranscriptSegment['source'] }) {
  if (source === '系统音频') return <Volume2 size={12} />
  return <Mic size={12} />
}

export function SessionTree({ records, selectedId, onSelect, query }: SessionTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set(records.map((r) => r.id)))

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const filteredRecords = query
    ? records.filter((r) =>
        r.revision.segments.some((s) => s.text.toLowerCase().includes(query.toLowerCase()))
      )
    : records

  return (
    <aside className="session-tree">
      {filteredRecords.map((record) => {
        const isOpen = expanded.has(record.id)
        const segments = query
          ? record.revision.segments.filter((s) => s.text.toLowerCase().includes(query.toLowerCase()))
          : record.revision.segments

        return (
          <div key={record.id} className="session-tree__group">
            <button
              className={`session-tree__session ${record.id === selectedId ? 'session-tree__session--active' : ''}`}
              onClick={() => { toggle(record.id); onSelect(record.id) }}
            >
              {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <FileText size={14} />
              <span className="session-tree__title">{record.title}</span>
              <span className={`record-status record-status--${record.status}`} />
            </button>
            {isOpen && segments.map((seg) => (
              <button
                key={seg.id}
                className={`session-tree__segment ${selectedId === record.id ? 'session-tree__segment--active' : ''}`}
                onClick={() => onSelect(record.id)}
              >
                <SegmentIcon source={seg.source} />
                <span className="session-tree__time">{formatTime(seg.startMs)}</span>
                <span className="session-tree__preview">{seg.text.slice(0, 30)}{seg.text.length > 30 ? '…' : ''}</span>
              </button>
            ))}
            {/* Notes as sub-nodes */}
            {isOpen && record.notes?.map((note) => (
              <div key={note.id} className="session-tree__note">
                <span className="session-tree__note-tag">{note.tag}</span>
                <span className="session-tree__note-preview">{note.content.slice(0, 25)}…</span>
              </div>
            ))}
            {record.status === 'processing' && segments.length === 0 && (
              <div className="session-tree__processing">
                <Clock size={12} /> 处理中...
              </div>
            )}
          </div>
        )
      })}
      {filteredRecords.length === 0 && (
        <div className="empty-state">
          <strong>没有匹配的会话</strong>
          <p>换一个关键词搜索，或导入新的音频。</p>
        </div>
      )}
    </aside>
  )
}
```

- [ ] **Step 2: Add SessionTree styles**

```css
.session-tree {
  min-width: 0;
  padding: var(--spacing-3);
  overflow: auto;
  border-right: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.session-tree__group { margin-bottom: var(--spacing-1); }
.session-tree__session {
  width: 100%; min-height: 36px; padding: var(--spacing-1) var(--spacing-2);
  display: flex; align-items: center; gap: var(--spacing-2);
  border: 0; border-radius: 0; color: var(--colors-brand-textSecondary);
  background: transparent; font-family: var(--typography-fontFamily-mono);
  font-size: var(--typography-fontSize-xs); cursor: pointer; text-align: left;
}
.session-tree__session:hover { background: var(--colors-brand-surfaceSubtle); }
.session-tree__session--active { color: var(--colors-brand-textPrimary); background: var(--colors-brand-surface); }
.session-tree__title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.session-tree__segment {
  width: 100%; min-height: 32px; padding: var(--spacing-1) var(--spacing-2) var(--spacing-1) var(--spacing-8);
  display: flex; align-items: center; gap: var(--spacing-2);
  border: 0; color: var(--colors-brand-textMuted); background: transparent;
  font-family: var(--typography-fontFamily-mono); font-size: 11px; cursor: pointer; text-align: left;
}
.session-tree__segment:hover { color: var(--colors-brand-textSecondary); }
.session-tree__time { font-variant-numeric: tabular-nums; min-width: 48px; }
.session-tree__preview { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.session-tree__note {
  padding: var(--spacing-1) var(--spacing-2) var(--spacing-1) var(--spacing-10);
  display: flex; gap: var(--spacing-2); font-family: var(--typography-fontFamily-mono); font-size: 10px;
  color: var(--colors-brand-textMuted);
}
.session-tree__note-tag { padding: 0 var(--spacing-1); border: 1px solid var(--colors-brand-border); }
.session-tree__processing {
  padding: var(--spacing-2) var(--spacing-2) var(--spacing-2) var(--spacing-8);
  display: flex; align-items: center; gap: var(--spacing-2);
  color: var(--colors-brand-paused); font-family: var(--typography-fontFamily-mono); font-size: 11px;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/SessionTree.tsx src/styles.css
git commit -m "feat: add SessionTree with expandable session-segment hierarchy"
```

### Task 8: Implement StatsBar

**Files:**
- Create: `src/components/StatsBar.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write StatsBar**

```tsx
import type { StatsSnapshot } from '../domain'

interface StatsBarProps {
  stats: StatsSnapshot
}

export function StatsBar({ stats }: StatsBarProps) {
  const maxMinutes = Math.max(...stats.hourlySlots.map((s) => s.minutes), 1)

  return (
    <footer className="stats-bar">
      <div className="stats-bar__row">
        <span className="eyebrow">📊 今天</span>
        <div className="stats-bar__blocks">
          {stats.hourlySlots.map((slot) => {
            const height = Math.max(2, Math.round((slot.minutes / maxMinutes) * 24))
            const isActive = slot.hour === new Date().getHours()
            return (
              <div
                key={slot.hour}
                className={`stats-bar__block ${isActive ? 'stats-bar__block--active' : ''}`}
                style={{ height: `${height}px` }}
                title={slot.title ? `${slot.hour}:00 · ${slot.minutes} 分钟 · ${slot.title}` : `${slot.hour}:00`}
              />
            )
          })}
        </div>
      </div>
      <div className="stats-bar__labels">
        {[0, 6, 12, 18, 23].map((h) => (
          <span key={h} className="stats-bar__label">{String(h).padStart(2, '0')}</span>
        ))}
      </div>
      <div className="stats-bar__summary">
        <span>本周 {stats.weekSessions} 会话 · {stats.weekMinutes} 分钟</span>
        <span className="stats-bar__divider">│</span>
        <span>本月 {stats.monthSessions} 会话 · {stats.monthMinutes} 分钟</span>
        <span className="stats-bar__divider">│</span>
        <span>累计 {stats.totalSessions} 会话 · {Math.floor(stats.totalMinutes / 60)} 时</span>
      </div>
    </footer>
  )
}
```

- [ ] **Step 2: Add StatsBar styles**

```css
.stats-bar {
  padding: var(--spacing-3) var(--spacing-6);
  border-top: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.stats-bar__row { display: flex; align-items: center; gap: var(--spacing-4); margin-bottom: var(--spacing-1); }
.stats-bar__blocks { display: flex; gap: 2px; align-items: flex-end; height: 28px; }
.stats-bar__block { width: 10px; border-radius: 2px; background: var(--colors-brand-border); transition: background 150ms; }
.stats-bar__block:hover { background: var(--colors-brand-textMuted); }
.stats-bar__block--active { background: var(--colors-brand-focus); }
.stats-bar__labels { display: flex; justify-content: space-between; padding: 0 var(--spacing-1); margin-bottom: var(--spacing-2); }
.stats-bar__label { font-family: var(--typography-fontFamily-mono); font-size: 9px; color: var(--colors-brand-textMuted); }
.stats-bar__summary { display: flex; gap: var(--spacing-3); font-family: var(--typography-fontFamily-mono); font-size: 11px; color: var(--colors-brand-textMuted); }
.stats-bar__divider { color: var(--colors-brand-border); }
```

- [ ] **Step 3: Commit**

```bash
git add src/components/StatsBar.tsx src/styles.css
git commit -m "feat: add StatsBar with 24h block chart and weekly/monthly totals"
```

### Task 9: Implement TimelineView page

**Files:**
- Modify: `src/components/TimelineView.tsx`
- Modify: `src/components/TranscriptView.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Rewrite TimelineView**

```tsx
import { useState, useMemo } from 'react'
import { Search, Upload } from 'lucide-react'
import { SessionTree } from './SessionTree'
import { TranscriptView } from './TranscriptView'
import { StatsBar } from './StatsBar'
import { demoStats } from '../data/demo'
import type { EvidenceRecord, TranscriptRevision } from '../domain'

interface TimelineViewProps {
  records: EvidenceRecord[]
  onRecordsChange: (records: EvidenceRecord[]) => void
  onNotice: (msg: string) => void
}

export function TimelineView({ records, onRecordsChange, onNotice }: TimelineViewProps) {
  const [selectedId, setSelectedId] = useState(records[0]?.id ?? '')
  const [query, setQuery] = useState('')

  const selectedRecord = useMemo(
    () => records.find((r) => r.id === selectedId) ?? records[0],
    [records, selectedId]
  )

  const handleRevisionChange = (revision: TranscriptRevision) => {
    onRecordsChange(
      records.map((r) =>
        r.id === selectedId ? { ...r, revision } : r
      )
    )
  }

  const handleImport = () => {
    onNotice('导入音频功能：选择本地音频文件。')
  }

  return (
    <main className="timeline-view">
      <header className="timeline-view__toolbar">
        <div className="search-field">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            aria-label="搜索转写"
            placeholder="搜索原话、来源或时间…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button className="button" onClick={handleImport}>
          <Upload size={16} />导入音频
        </button>
      </header>

      <div className="timeline-view__content">
        <SessionTree
          records={records}
          selectedId={selectedId}
          onSelect={setSelectedId}
          query={query}
        />
        {selectedRecord && (
          <TranscriptView
            record={selectedRecord}
            query={query}
            onRevisionChange={handleRevisionChange}
            onNotice={onNotice}
          />
        )}
      </div>

      <StatsBar stats={demoStats} />
    </main>
  )
}
```

- [ ] **Step 2: Add TimelineView styles**

```css
.timeline-view {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  overflow: hidden;
  border: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-surface);
}
.timeline-view__toolbar {
  padding: var(--spacing-3) var(--spacing-4);
  display: flex; gap: var(--spacing-3);
  border-bottom: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.timeline-view__toolbar .search-field { flex: 1; }
.timeline-view__content {
  min-height: 0;
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  overflow: hidden;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/TimelineView.tsx src/components/TranscriptView.tsx src/styles.css
git commit -m "feat: implement TimelineView with SessionTree, search, and StatsBar"
```

---

## Chunk 6: 词典页面 (DictionaryView)

### Task 10: Implement DictionaryView

**Files:**
- Modify: `src/components/DictionaryView.tsx`
- Create: `src/components/DictionaryView.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write DictionaryView**

```tsx
import { useState } from 'react'
import { Search, Plus, Check, X } from 'lucide-react'
import type { DictionaryCategory, DictionaryEntry } from '../domain'

interface DictionaryViewProps {
  categories: DictionaryCategory[]
  entries: DictionaryEntry[]
  onNotice: (msg: string) => void
}

export function DictionaryView({ categories, entries, onNotice }: DictionaryViewProps) {
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
```

- [ ] **Step 2: Add DictionaryView styles**

```css
.dictionary-view {
  min-height: 0; display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  overflow: hidden; border: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-surface);
}
.dictionary-view__header {
  padding: var(--spacing-4) var(--spacing-6);
  display: flex; justify-content: space-between; align-items: center;
  border-bottom: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.dictionary-view__header h1 { margin: var(--spacing-1) 0 0; font-size: var(--typography-fontSize-lg); }
.dictionary-view__actions { display: flex; gap: var(--spacing-2); }
.dictionary-view__scope {
  padding: var(--spacing-1) var(--spacing-2);
  border: 1px solid var(--colors-brand-border);
  color: var(--colors-brand-textPrimary); background: var(--colors-brand-canvas);
  font-family: var(--typography-fontFamily-mono); font-size: var(--typography-fontSize-xs);
}
.dictionary-view__body {
  min-height: 0; display: grid;
  grid-template-columns: 200px minmax(0, 1fr) 240px;
  overflow: hidden;
}
.dictionary-view__categories {
  overflow: auto; padding: var(--spacing-3);
  border-right: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.dictionary-category {
  width: 100%; padding: var(--spacing-2) var(--spacing-3);
  display: flex; justify-content: space-between; align-items: center;
  border: 0; color: var(--colors-brand-textSecondary); background: transparent;
  font-family: var(--typography-fontFamily-mono); font-size: var(--typography-fontSize-xs); cursor: pointer;
}
.dictionary-category:hover { background: var(--colors-brand-surfaceSubtle); }
.dictionary-category--active { color: var(--colors-brand-textPrimary); background: var(--colors-brand-surface); }
.dictionary-category__count { font-size: 10px; color: var(--colors-brand-textMuted); }
.dictionary-view__entries { overflow: auto; padding: var(--spacing-4); }
.dictionary-entries { display: grid; gap: var(--spacing-1); }
.dictionary-entry {
  width: 100%; padding: var(--spacing-2) var(--spacing-3);
  display: flex; align-items: center; gap: var(--spacing-3);
  border: 1px solid transparent; color: var(--colors-brand-textSecondary); background: transparent;
  font-size: var(--typography-fontSize-sm); cursor: pointer; text-align: left;
}
.dictionary-entry:hover { background: var(--colors-brand-surfaceSubtle); }
.dictionary-entry--active { border-color: var(--colors-brand-borderStrong); background: var(--colors-brand-surface); }
.dictionary-entry__status { color: var(--colors-brand-available); }
.dictionary-entry__aliases { color: var(--colors-brand-textMuted); font-size: var(--typography-fontSize-xs); margin-left: auto; }
.dictionary-view__detail {
  overflow: auto; padding: var(--spacing-4);
  border-left: 1px solid var(--colors-brand-border);
  background: var(--colors-brand-canvas);
}
.dictionary-view__detail h3 { margin: 0 0 var(--spacing-4); font-size: var(--typography-fontSize-lg); }
.dictionary-detail__fields { display: grid; gap: var(--spacing-3); }
.dictionary-detail__field { }
.dictionary-detail__field label { display: block; color: var(--colors-brand-textMuted); font-family: var(--typography-fontFamily-mono); font-size: 10px; text-transform: uppercase; letter-spacing: 0.08em; }
.dictionary-detail__field span { font-size: var(--typography-fontSize-sm); }
.dictionary-detail__actions { margin-top: var(--spacing-6); display: flex; gap: var(--spacing-2); }
.dictionary-view__footer {
  padding: var(--spacing-3) var(--spacing-6);
  border-top: 1px solid var(--colors-brand-border);
  color: var(--colors-brand-textMuted); font-size: var(--typography-fontSize-xs);
  background: var(--colors-brand-canvas);
}
```

- [ ] **Step 3: Write tests**

```tsx
// src/components/DictionaryView.test.tsx
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { DictionaryView } from './DictionaryView'
import { demoCategories, demoEntries } from '../data/demo'

describe('DictionaryView', () => {
  it('renders categories', () => {
    render(<DictionaryView categories={demoCategories} entries={demoEntries} onNotice={() => {}} />)
    expect(screen.getByText('人名')).toBeInTheDocument()
    expect(screen.getByText('地名')).toBeInTheDocument()
  })

  it('shows entries for selected category', async () => {
    const user = userEvent.setup()
    render(<DictionaryView categories={demoCategories} entries={demoEntries} onNotice={() => {}} />)
    await user.click(screen.getByText('人名'))
    expect(screen.getByText('张伟')).toBeInTheDocument()
  })

  it('shows entry detail on click', async () => {
    const user = userEvent.setup()
    render(<DictionaryView categories={demoCategories} entries={demoEntries} onNotice={() => {}} />)
    await user.click(screen.getByText('张伟'))
    expect(screen.getByText('zhāng wěi')).toBeInTheDocument()
  })
})
```

- [ ] **Step 4: Run tests**

```bash
npm test -- --run src/components/DictionaryView.test.tsx
```

- [ ] **Step 5: Commit**

```bash
git add src/components/DictionaryView.tsx src/components/DictionaryView.test.tsx src/styles.css
git commit -m "feat: implement DictionaryView with category/entry/detail panels"
```

---

## Chunk 7: 设置弹窗 (SettingsModal)

### Task 11: Implement SettingsModal with 4 tabs

**Files:**
- Modify: `src/components/SettingsModal.tsx`
- Create: `src/components/RecordingSettings.tsx`
- Create: `src/components/AsrSettings.tsx`
- Create: `src/components/ModelManager.tsx`
- Create: `src/components/AboutTab.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write SettingsModal**

```tsx
import { useState } from 'react'
import { Modal } from './Modal'
import { TabBar } from './TabBar'
import { RecordingSettings } from './RecordingSettings'
import { AsrSettings } from './AsrSettings'
import { ModelManager } from './ModelManager'
import { AboutTab } from './AboutTab'

const TABS = [
  { id: 'recording', label: '录音设置' },
  { id: 'asr', label: 'ASR 设置' },
  { id: 'models', label: '模型' },
  { id: 'about', label: '关于' },
]

interface SettingsModalProps {
  open: boolean
  onClose: () => void
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState('recording')

  return (
    <Modal open={open} onClose={onClose} title="设置">
      <div className="settings-layout">
        <nav className="settings-nav">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              className={`settings-nav__item ${tab.id === activeTab ? 'settings-nav__item--active' : ''}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="settings-content">
          {activeTab === 'recording' && <RecordingSettings />}
          {activeTab === 'asr' && <AsrSettings />}
          {activeTab === 'models' && <ModelManager />}
          {activeTab === 'about' && <AboutTab />}
        </div>
      </div>
    </Modal>
  )
}
```

- [ ] **Step 2: Add settings layout styles**

```css
.settings-layout { display: grid; grid-template-columns: 160px minmax(0, 1fr); min-height: 400px; }
.settings-nav { padding: var(--spacing-4) 0; border-right: 1px solid var(--colors-brand-border); }
.settings-nav__item {
  width: 100%; padding: var(--spacing-2) var(--spacing-4);
  display: block; border: 0; border-left: 2px solid transparent;
  color: var(--colors-brand-textSecondary); background: transparent;
  font-family: var(--typography-fontFamily-mono); font-size: var(--typography-fontSize-xs);
  text-align: left; cursor: pointer;
}
.settings-nav__item:hover { color: var(--colors-brand-textPrimary); background: var(--colors-brand-surfaceSubtle); }
.settings-nav__item--active { color: var(--colors-brand-textPrimary); border-left-color: var(--colors-brand-textPrimary); }
.settings-content { padding: var(--spacing-6); overflow: auto; }
```

- [ ] **Step 3: Write tab content components**

Create stub implementations with the design from our ASCII wireframes. Each tab is a self-contained component:

```tsx
// src/components/RecordingSettings.tsx
// 双声道分轨用于提升 ASR 准确率（左右声道独立音频流），
// 转写结果中的说话人标注按人名（来自词典匹配/手动标注/未知），
// 而非按声道。参见 LiveSegment.speaker 字段。
export function RecordingSettings() { /* ... capture mode, IM detection, audio format, storage */ }

// src/components/AsrSettings.tsx
// ASR 设置 Tab 包含：
//   1. Provider 选择器（SenseVoice / Whisper / Qwen3-ASR）
//   2. 语言与行为（语言 / 自动转写 / 线程数）
//   3. VAD 设置（开关 / 最小语音长度 / 静音阈值）
//   4. Provider 专属选项（动态，如 ITN）
//   5. 声纹库（已注册声纹列表 + 注册新声纹 + 关联词典词条）
export function AsrSettings() { /* ... */ }

// src/components/ModelManager.tsx
export function ModelManager() { /* ... installed models, available models, download progress */ }

// src/components/AboutTab.tsx
export function AboutTab() { /* ... version, runtime, licenses */ }
```

Full implementations are in the ASCII wireframes from the design discussion. See plan appendix for reference.

- [ ] **Step 4: Commit**

```bash
git add src/components/SettingsModal.tsx src/components/RecordingSettings.tsx src/components/AsrSettings.tsx src/components/ModelManager.tsx src/components/AboutTab.tsx src/styles.css
git commit -m "feat: implement SettingsModal with 4-tab sidebar layout"
```

---

## Chunk 8: Final Integration & Test Migration

### Task 12: Polish and verify

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Remove old component files**

Delete files that are fully replaced:
- `src/components/RecorderBar.tsx` (merged into LiveCapture)
- `src/components/RecordList.tsx` (replaced by SessionTree)
- `src/components/SettingsView.tsx` (replaced by SettingsModal)

- [ ] **Step 2: Run full test suite**

```bash
npm test -- --run
```

- [ ] **Step 3: Run type check**

```bash
npx tsc --noEmit
```

- [ ] **Step 4: Run build**

```bash
npm run build
```

- [ ] **Step 5: Final commit**

```bash
git add -A src/
git commit -m "feat: complete UI redesign — 4-page shell, live capture, timeline tree, dictionary, settings modal"
```

---

## Appendix: Design Reference

See the ASCII wireframes from the design discussion for detailed component layouts:

- **录音页面**: streaming ASR console with note panel
- **时间线页面**: SessionTree + TranscriptView + StatsBar
- **词典页面**: category/entry/detail three-panel layout
- **设置弹窗**: 4-tab modal with left sidebar navigation

---

## Verification Checklist

- [ ] `npm test -- --run` — all tests pass (0 failures)
- [ ] `npx tsc --noEmit` — no type errors
- [ ] `npm run build` — production build succeeds
- [ ] Sidebar navigation switches between 4 pages
- [ ] Settings modal opens/closes with Esc and overlay click
- [ ] SessionTree expands/collapses and filters by search
- [ ] StatsBar renders 24h blocks with correct colors
- [ ] DictionaryView shows categories and entries
- [ ] LiveCapture shows idle state and recording state
- [ ] NotePanel allows adding and deleting notes
- [ ] No console.log in production code
- [ ] All design tokens used (no hardcoded colors/spacing)
- [ ] Responsive breakpoints preserved (1023/767/520)