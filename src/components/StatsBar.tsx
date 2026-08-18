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