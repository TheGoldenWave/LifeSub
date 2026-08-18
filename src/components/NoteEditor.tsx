import { useState } from 'react'
import type { CaptureNote, NoteTag, LiveSegment } from '../domain'

interface NoteEditorProps {
  onSave: (note: CaptureNote) => void
  onCancel: () => void
  segments: LiveSegment[]
}

const TAGS: NoteTag[] = ['待办', '备忘', '问题', '决定']

export function NoteEditor({ onSave, onCancel, segments: _segments }: NoteEditorProps) {
  const [content, setContent] = useState('')
  const [tag, setTag] = useState<NoteTag>('备忘')

  const handleSave = () => {
    if (!content.trim()) return
    onSave({
      id: `note-${Date.now()}`,
      content: content.trim(),
      timestampMs: Date.now() % 3600000,
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