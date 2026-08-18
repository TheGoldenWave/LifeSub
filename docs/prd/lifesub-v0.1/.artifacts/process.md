---
stage: v0.1-design-governed
last_updated: 2026-08-18
source: codex-goal
---

# LifeSub V0.1 进度

- 当前阶段：v0.1-design-governed
- 已交付：Tauri + React 桌面壳、本地 SQLite Evidence Core、录音状态机、音频导入与 hash、append-only revision、中文搜索、Evidence URI、Markdown 导出、设置页与浏览器演示模式。
- 验证证据：前端单测 6/6、Rust 单测 6/6、Playwright 验收 1/1、Vite 生产构建通过、Tauri desktop feature 编译通过、无 `console.log`。
- 视觉验收：`output/playwright/lifesub-v0.1-desktop.png`。
- 设计治理：已参考 `/Users/goldenwave/Downloads/DESIGN.md` 产出根目录 `design.md`，完成深色单色、锐利边界、无阴影、等宽操作层和 Token 自动生成治理。
- 治理截图：`output/playwright/lifesub-design-governance-desktop.png`、`lifesub-design-governance-tablet.png`、`lifesub-design-governance-mobile.png`。
- 重新打包：`src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64-signed.dmg`；DMG checksum 与镜像内 `.app` bundle 签名均验证通过。
- GitHub 同步：2026-08-18 已完成提交前安全扫描与质量检查，当前项目快照准备同步至 `origin/main`。
- README 更新：2026-08-18 已按 V0.1 实际交付、V0.2 本地 ASR 计划和后续云端 ASR 方向重整产品入口、开发指南、架构分层与路线图口径。
- 下一步：接入真实本地 ASR Provider，并实现 ScreenCaptureKit + AVAudioEngine 双路 Capture Adapter。
- 阻塞项：无首版阻塞；原生双路录音仍需 macOS 权限、签名和实机长时测试。
