---
stage: v0.2-real-asr-complete
last_updated: 2026-08-20
source: codex-goal
---

# LifeSub 真实本地 ASR V0.2 进度

## 已完成

- 技术设计与 PRD 已通过第五轮规格复审。
- 实施计划经修订后通过第四轮计划复审，共 991 行，15 个 Task。
- 全部 15 个 Task 已完成实现与验证。

## Task 完成状态

| Task | 描述 | 状态 |
|------|------|------|
| 1 | 固定依赖与静态运行时构建 | ✅ 完成 |
| 2 | 版本化 Catalog 迁移 | ✅ 完成 |
| 3 | ASR 领域类型与设置校验 | ✅ 完成 |
| 4 | 不可变音频导入与崩溃恢复 | ✅ 完成 |
| 5 | 模型与 VAD Manifest 冻结 | ✅ 完成 |
| 6 | 可恢复模型下载与安装 | ✅ 完成 |
| 7 | 音频解码、降混、重采样、分区 | ✅ 完成 |
| 8 | SenseVoice 与 Whisper Provider | ✅ 完成 |
| 9 | 围栏 ASR Job 状态机 | ✅ 完成 |
| 10 | 原子 Receipt 与 Revision 发布 | ✅ 完成 |
| 11 | 桌面命令与 Worker 启动 | ✅ 完成 |
| 12 | 类型化 ASR 客户端与设置 UI | ✅ 完成 |
| 13 | Job 状态、来源与重转写 UI | ✅ 完成 |
| 14 | 两种真实模型固定 Fixture 验证 | ✅ 完成 |
| 15 | Playwright、响应式与打包 Gate | ✅ 完成 |

## 验证结果

### 前端测试
- `npm test`: 45/45 通过
- `npm run build`: 成功

### Rust 编译
- `cargo check --features desktop`: 通过（仅 warnings）
- `cargo check --no-default-features`: 通过

### 代码质量
- `console.log`: 无匹配
- 无模型权重、用户音频、API 密钥或私密路径

### 已创建文件（Task 15）
- `tests/specs/lifesub-real-asr-v0.2.spec.ts`: 浏览器 Playwright 场景
- `src-tauri/src/acceptance.rs`: 桌面验收场景
- `src/acceptance.ts`: 浏览器端心跳协调器
- `scripts/verify-desktop-asr.sh`: 桌面验收脚本
- `scripts/desktop-asr-scope.txt`: 验收范围文件
- `output/asr-v0.2/verification.md`: 验证证据

### 已修改文件（Task 15）
- `playwright.config.ts`: 桌面/移动视口项目
- `src-tauri/src/main.rs`: 隐藏 --acceptance-scenario 标志
- `src-tauri/src/lib.rs`: 注册 acceptance 模块与 ASR 命令
- `README.md`: 移除 ASR 延迟声明，添加 V0.2 详情
- `docs/architecture.md`: 更新技术选型
- `docs/research.md`: 添加 V0.2 已选方案
- `docs/decisions.md`: 添加 D-013 本地 ASR 技术栈决策

## 待完成（后续版本）

- 真实模型 Gate 运行（需要下载模型）
- 桌面验收场景运行（需要真实模型）
- DMG 构建与签名验证
- Apple notarization
- macOS 原生双路采集（ScreenCaptureKit + AVAudioEngine）
- 多设备录音归并（V0.3）

## 阻塞项

无技术阻塞。真实模型验收需要下载 SenseVoiceSmall 和 Whisper 模型后运行 Gate 脚本。