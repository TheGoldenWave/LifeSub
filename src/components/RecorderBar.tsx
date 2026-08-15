import { CirclePause, CirclePlay, Disc3, Square } from 'lucide-react'
import type { CaptureState } from '../domain'

const stateLabels: Record<CaptureState, string> = {
  idle: '准备就绪',
  recording: '正在记录',
  paused: '已暂停',
  stopped: '记录已封存',
}

interface RecorderBarProps {
  state: CaptureState
  onStateChange: (state: CaptureState) => void
}

export function RecorderBar({ state, onStateChange }: RecorderBarProps) {
  return (
    <section className={`recorder recorder--${state}`} aria-label="录音控制">
      <div className="recorder__status">
        <span className="recorder__pulse" aria-hidden="true" />
        <div>
          <span className="eyebrow">Capture</span>
          <strong>{stateLabels[state]}</strong>
        </div>
      </div>
      <div className="recorder__actions">
        {(state === 'idle' || state === 'stopped') && (
          <button className="button button--primary" onClick={() => onStateChange('recording')}><Disc3 size={17} />开始记录</button>
        )}
        {state === 'recording' && (
          <>
            <button className="button" onClick={() => onStateChange('paused')}><CirclePause size={17} />暂停</button>
            <button className="button button--danger" onClick={() => onStateChange('stopped')}><Square size={15} />停止</button>
          </>
        )}
        {state === 'paused' && (
          <>
            <button className="button button--primary" onClick={() => onStateChange('recording')}><CirclePlay size={17} />继续</button>
            <button className="button button--danger" onClick={() => onStateChange('stopped')}><Square size={15} />停止</button>
          </>
        )}
      </div>
    </section>
  )
}
