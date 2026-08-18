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