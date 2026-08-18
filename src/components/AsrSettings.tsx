import { demoVoiceprints } from '../data/demo'

export function AsrSettings() {
  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ASR</span>
      <h1>ASR 设置</h1>

      <section className="settings-section">
        <h2>Provider</h2>
        <div className="setting-row">
          <label>当前 Provider</label>
          <select className="dictionary-view__scope">
            <option value="sensevoice">SenseVoice（推荐）</option>
            <option value="whisper">Whisper</option>
            <option value="qwen3-asr">Qwen3-ASR</option>
          </select>
        </div>
      </section>

      <section className="settings-section">
        <h2>语言与行为</h2>
        <div className="setting-row">
          <label>识别语言</label>
          <select className="dictionary-view__scope">
            <option value="zh">中文</option>
            <option value="en">English</option>
            <option value="auto">自动检测</option>
          </select>
        </div>
        <div className="setting-row">
          <label>自动转写</label>
          <span className="status-pill">启用</span>
        </div>
        <div className="setting-row">
          <label>线程数</label>
          <span>4</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>VAD 设置</h2>
        <div className="setting-row">
          <label>VAD</label>
          <span className="status-pill">启用</span>
        </div>
        <div className="setting-row">
          <label>最小语音长度</label>
          <span>300 ms</span>
        </div>
        <div className="setting-row">
          <label>静音阈值</label>
          <span>800 ms</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>Provider 专属选项</h2>
        <div className="setting-row">
          <label>ITN（逆文本正则化）</label>
          <span className="status-pill">启用</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>声纹库</h2>
        <p>已注册的说话人声纹，用于自动识别转写中的说话人。</p>
        {demoVoiceprints.length === 0 ? (
          <p className="empty-state">暂无注册声纹，录音时点击未知说话人即可保存。</p>
        ) : (
          <div className="voiceprint-list">
            {demoVoiceprints.map((vp) => (
              <div key={vp.id} className="voiceprint-card">
                <div className="voiceprint-card__info">
                  <strong>{vp.name}</strong>
                  <small>{vp.sampleCount} 个样本 · 更新于 {vp.updatedAt.slice(0, 10)}</small>
                  {vp.dictionaryEntryId && <span className="status-pill">关联词典</span>}
                </div>
                <div className="voiceprint-card__actions">
                  <button className="text-button">重命名</button>
                  <button className="text-button">删除</button>
                </div>
              </div>
            ))}
          </div>
        )}
        <button className="text-button" style={{ marginTop: 'var(--spacing-2)' }}>+ 注册新声纹</button>
      </section>
    </div>
  )
}