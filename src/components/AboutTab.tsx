export function AboutTab() {
  return (
    <div className="settings-tab-content">
      <span className="eyebrow">ABOUT</span>
      <h1>关于 LifeSub</h1>

      <section className="settings-section">
        <div className="setting-row">
          <label>版本</label>
          <span>0.2.0</span>
        </div>
        <div className="setting-row">
          <label>运行时</label>
          <span>Tauri + Rust</span>
        </div>
        <div className="setting-row">
          <label>前端</label>
          <span>React 19 + TypeScript</span>
        </div>
        <div className="setting-row">
          <label>ASR 引擎</label>
          <span>sherpa-onnx</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>本地优先承诺</h2>
        <p>所有录音、转写与声纹数据均存储在本机，云端处理默认关闭。</p>
      </section>
    </div>
  )
}