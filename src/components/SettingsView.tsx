import { CloudOff, Database, HardDrive, ShieldCheck } from 'lucide-react'

export function SettingsView() {
  return (
    <main className="settings-view">
      <header><span className="eyebrow">Local Evidence</span><h1>设置</h1><p>首版坚持本地优先。Provider 与数据去向在每次处理时单独记录。</p></header>
      <section className="settings-section">
        <h2>处理 Provider</h2>
        <div className="setting-row"><Database /><div><strong>本地演示 ASR</strong><p>当前用于验证完整 Evidence 流程；保留可替换的本地模型接口。</p></div><span className="status-pill">启用</span></div>
        <div className="setting-row"><CloudOff /><div><strong>云端处理默认关闭</strong><p>云端 ASR 与云端校对需要分别授权，不会沿用同一许可。</p></div><span className="status-pill status-pill--quiet">关闭</span></div>
      </section>
      <section className="settings-section">
        <h2>数据与隐私</h2>
        <div className="setting-row"><HardDrive /><div><strong>本机应用数据目录</strong><p>SQLite Catalog、导入音频与 Markdown 派生内容均保存在本机。</p></div></div>
        <div className="setting-row"><ShieldCheck /><div><strong>Evidence 可追溯</strong><p>原始 revision 不被覆盖，访问、导出、删除和撤回将进入审计链。</p></div></div>
      </section>
    </main>
  )
}
