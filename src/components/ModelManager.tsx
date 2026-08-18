export function ModelManager() {
  return (
    <div className="settings-tab-content">
      <span className="eyebrow">MODELS</span>
      <h1>模型</h1>

      <section className="settings-section">
        <h2>已安装</h2>
        <div className="model-card">
          <strong>SenseVoice Small</strong>
          <small>v1.0 · 约 120 MB · 下载于 2026-08-12</small>
          <span className="status-pill">已安装</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>可安装</h2>
        <div className="model-card">
          <strong>Whisper Large v3</strong>
          <small>约 1.5 GB</small>
          <button className="button">下载</button>
        </div>
        <div className="model-card">
          <strong>Qwen3-ASR</strong>
          <small>约 800 MB</small>
          <button className="button">下载</button>
        </div>
      </section>
    </div>
  )
}