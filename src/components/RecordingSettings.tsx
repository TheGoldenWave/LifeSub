export function RecordingSettings() {
  return (
    <div className="settings-tab-content">
      <span className="eyebrow">RECORDING</span>
      <h1>录音设置</h1>

      <section className="settings-section">
        <h2>捕获模式</h2>
        <div className="setting-row">
          <label>默认模式</label>
          <select className="dictionary-view__scope">
            <option value="smart">智能路由（推荐）</option>
            <option value="mic-only">仅麦克风</option>
            <option value="system-only">仅系统音频</option>
          </select>
        </div>
        <div className="setting-row">
          <label>IM 通话检测</label>
          <span className="status-pill">启用</span>
          <small>微信 / 钉钉 / 飞书 / Teams / Zoom / QQ</small>
        </div>
        <div className="setting-row">
          <label>检测响应时间</label>
          <span>3 秒</span>
        </div>
        <div className="setting-row">
          <label>通话结束恢复</label>
          <span>5 秒</span>
        </div>
      </section>

      <section className="settings-section">
        <h2>音频格式</h2>
        <div className="setting-row">
          <label>编码</label>
          <span>WAV 16-bit PCM</span>
        </div>
        <div className="setting-row">
          <label>采样率</label>
          <span>16 kHz</span>
        </div>
        <div className="setting-row">
          <label>默认声道</label>
          <span>单声道（非通话）</span>
        </div>
        <div className="setting-row">
          <label>通话声道</label>
          <span>双声道（系统 + 麦克风）</span>
        </div>
        <small>双声道分轨仅用于提升 ASR 准确率，说话人标注按人名（声纹/词典/手动），不按声道。</small>
      </section>

      <section className="settings-section">
        <h2>存储</h2>
        <div className="setting-row">
          <label>录音目录</label>
          <code>~/.lifesub/recordings/</code>
        </div>
      </section>
    </div>
  )
}