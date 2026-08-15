# LifeSub 最新安装包固定路径设计

## 目标

为本机直接安装提供一个不会随版本号或 CPU 架构变化的 DMG 路径：

```text
output/installers/LifeSub-latest.dmg
```

当前 Tauri 产物位于 `src-tauri/target/release/bundle/dmg/`，文件名包含版本号、架构和签名状态，因此升级后路径会变化。

## 方案

新增 `scripts/publish-latest-installer.mjs`，并通过 `package.json` 暴露 `npm run installer:latest`。脚本只负责将已经生成并签名的 DMG 发布到固定入口，不负责构建、签名或修改原始安装包。

脚本导出异步函数 `publishLatestInstaller({ sourceDir, targetPath, architecture })`，供自动测试注入隔离目录。成功时返回 `{ sourcePath, targetPath }` 两个绝对路径；失败时抛出带可读原因的 `Error`。命令行入口捕获异常并负责格式化输出和退出码，不接受命令行参数。默认值为：

- `sourceDir`：`src-tauri/target/release/bundle/dmg/`
- `targetPath`：`output/installers/LifeSub-latest.dmg`
- `architecture`：当前 Node.js 进程架构；`arm64` 对应 Tauri 文件名中的 `aarch64`，`x64` 对应 `x64`

架构匹配只接受文件名后缀 `_aarch64-signed.dmg` 或 `_x64-signed.dmg`，不会使用包含关系做模糊匹配。`architecture` 不是 `arm64` 或 `x64` 时立即抛出“不支持的架构”错误。

脚本执行时：

1. 在 Tauri DMG 输出目录中查找符合当前架构的 `*-signed.dmg`，不接受其他架构或未签名文件。
2. 按修改时间选择最新文件；修改时间相同时，按完整文件名的字典序选择最后一个，保证结果确定。
3. 创建 `output/installers/` 目录。
4. 先复制到同目录带进程 ID 后缀的临时文件，再原子替换 `LifeSub-latest.dmg`。
5. 输出固定安装包的绝对路径和来源文件名。

使用真实副本而不是符号链接。这样即使清理 `src-tauri/target/`，固定安装包仍可直接双击安装。版本化原包继续保留在 Tauri 输出目录，便于追溯。

## 边界与失败处理

- 没有已签名 DMG 时立即失败，不回退到未签名安装包。
- 没有匹配本机架构的已签名 DMG 时立即失败，不发布其他架构的文件。
- `latest` 表示最近生成的匹配产物，而不是语义版本号最高的产物；本地连续构建可能重复使用同一版本号，因此以修改时间为准。
- 无来源、目录创建失败、元数据读取失败、复制失败或原子替换失败时均返回非零退出码。只在完整临时副本准备好后替换目标，因此已有 `LifeSub-latest.dmg` 在所有失败路径中保持不变。
- 临时文件在成功和失败后都必须清理。本次命令面向单用户本机发布，不承诺并发执行语义。
- 成功时退出码为 `0`，标准输出固定为两行：`Latest installer: <absolute-path>` 和 `Source: <filename>`。失败时退出码为 `1`，标准错误为 `Failed to publish latest installer: <reason>`。
- 固定副本是本机构建产物，不提交到 Git。
- `.gitignore` 增加 `output/installers/*.dmg`，但不忽略同目录未来可能加入的说明或校验文件。
- 本次不改变 Tauri 的版本号、签名方式或 DMG 生成流程。

## 验证

- 核心函数单元测试覆盖无来源、不支持架构、错误架构、单个匹配产物、多个产物、修改时间相同和重复执行；验证成功返回值、异常原因、旧目标保护和临时文件清理。
- CLI 子进程集成测试分别覆盖成功与失败，验证退出码及标准输出、标准错误格式。
- 用当前 `LifeSub_0.1.0_aarch64-signed.dmg` 执行整理命令。
- 校验固定副本与来源文件的 SHA-256 一致。
- 确认固定路径是可读的普通 DMG 文件，并在 Codex 中提供该绝对路径供 Finder 直接打开。
