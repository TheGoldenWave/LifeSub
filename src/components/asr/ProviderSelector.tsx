import { Brain, Mic } from 'lucide-react'
import type { AsrProviderKind } from '../../domain'

interface ProviderSelectorProps {
  provider: AsrProviderKind
  onChange: (provider: AsrProviderKind) => void
}

export function ProviderSelector({ provider, onChange }: ProviderSelectorProps) {
  return (
    <div className="provider-selector" role="radiogroup" aria-label="选择 Provider">
      <button
        className={`provider-selector__option ${provider === 'sense_voice' ? 'provider-selector__option--active' : ''}`}
        role="radio"
        aria-checked={provider === 'sense_voice'}
        aria-label="SenseVoice"
        onClick={() => onChange('sense_voice')}
      >
        <Brain size={16} />
        <span>SenseVoice</span>
      </button>
      <button
        className={`provider-selector__option ${provider === 'whisper' ? 'provider-selector__option--active' : ''}`}
        role="radio"
        aria-checked={provider === 'whisper'}
        aria-label="Whisper"
        onClick={() => onChange('whisper')}
      >
        <Mic size={16} />
        <span>Whisper</span>
      </button>
    </div>
  )
}