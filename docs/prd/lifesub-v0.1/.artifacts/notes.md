# LifeSub V0.1 实施记录

- 2026-08-15：仓库已存在未提交的产品文档重构，开发过程必须保留这些用户变更。
- 2026-08-15：项目命令由 RTK 安全包装器接管，复杂检索需使用系统绝对路径或拆分执行。
- 2026-08-15：SQLite FTS5 默认 `unicode61` 会将连续中文长句作为整段 token，无法满足短语子串搜索；首版改用 `trigram` tokenizer。
- 2026-08-15：音频导入服务必须幂等确保父 CaptureSession 已登记，否则 Chunk 外键会因调用顺序隐式依赖而失败。
- 2026-08-15：为了让 Evidence Core 可独立测试，Tauri runtime 使用 `desktop` feature 隔离；`build.rs` 也必须同步检查 feature，否则纯 Core 测试仍会错误触发 Tauri 生成流程。
- 2026-08-15：本机 rustup 曾因并发组件恢复出现临时文件重命名冲突；验证时固定 `RUSTC` / `RUSTDOC` 到实际工具链二进制可绕过代理更新。
- 2026-08-15：设计治理采用“深色 Evidence Terminal”而非直接复制 xAI；单色为主，功能色只用于录音、暂停、可用、错误和焦点。
- 2026-08-15：Design Token 通过 `scripts/generate-design-tokens.mjs` 从 `docs/design/tokens/base.json` 生成 `src/design-tokens.css`，避免组件手工复制色值。
- 2026-08-15：Tauri 默认 release 产物只有 linker-level ad-hoc 签名，`Info.plist` 与 Resources 未封装签名；必须对整个 `.app` 执行 bundle ad-hoc 签名后重新生成 DMG，并挂载验证镜像内应用。
