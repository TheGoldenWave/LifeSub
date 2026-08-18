# LifeSub 多设备 Evidence 合并 V0.3 规划

## 目标

将 Mac、手机、手表等设备对同一事件产生的多份录音和 ASR 结果，治理为可解释、可回滚、可重新计算的合并版本，并生成最终合并 ASR Markdown 文档。

## 产品边界

包含音频来源归并、设备时间校准、重叠/缺口识别、音频质量评估、ASR 时间对齐、冲突标注、合并 Revision 和 Evidence 追溯。

不包含会议纪要、主题/决定/行动项抽取、Project/Matter 状态、长期知识治理、自动删除来源或复杂多人授权。

## 核心对象

- `DeviceSource`：设备、录音应用、采样率、时钟信息、来源许可和健康状态。
- `MergeCandidateSet`：被判断为同一事件的 Physical Audio Chunk 集合及归并依据。
- `ClockCalibration`：设备时间偏移、漂移、不确定范围和校准方法。
- `AudioOverlap`：来源之间的重叠、缺口、重复和质量比较结果。
- `TranscriptAlignment`：原始 ASR Segment 的时间对齐及来源映射。
- `MergeRevision`：新的不可变合并音频/ASR Revision，包含规则版本和全部来源关系。

## 治理规则

1. 原始录音和每台设备的独立 ASR 永久保留，合并不执行覆盖更新。
2. 合并过程先确定性校准时间，再处理重复、重叠和缺口。
3. 音频来源选择需结合信噪比、连续性、削波、采样率和设备健康信息。
4. ASR 冲突必须保留候选来源和冲突原因；不能静默选取单一文本。
5. 最终 Segment 必须关联设备、Chunk、原始 Revision、原始时间范围、合并时间范围和规则版本。
6. 合并结果失败或被撤回时，所有原始 Evidence 仍可读、可搜索、可重新合并。
7. 最终 Markdown 是可再生投影，不是独立事实源。

## 分阶段实施

### V0.3-A：来源和时间模型

- 扩展录音/Chunk provenance，记录设备标识和采集时钟。
- 建立同一事件的来源候选集和人工确认入口。
- 实现确定性时钟偏移估计、漂移记录和不确定范围。

### V0.3-B：音频与 ASR 对齐

- 实现重叠、缺口、重复和音频质量分析。
- 将各来源 ASR Segment 映射到统一时间轴。
- 保存 ASR 冲突、来源优先级和治理诊断。

### V0.3-C：合并 Revision 与发布

- 生成新的合并 Audio/Transcript Revision。
- 更新 FTS 和 `lifesub://` Evidence Ref，使最终片段可定位且可追溯。
- 生成最终合并 ASR Markdown，并支持重新计算和回滚到来源版本。

## 验收标准

- 同一事件的两台以上设备可以形成一个明确的 `MergeCandidateSet`。
- 时钟偏移和不确定范围有持久化证据，重新运行结果稳定。
- 重叠和缺口不会导致时间轴重复、倒退或静默丢失。
- 合并结果是新 Revision，旧音频和旧 ASR Revision 未被修改。
- 任一最终 Segment 可通过 `lifesub://` 定位到合并时间范围，并继续追溯到至少一个原始设备 Chunk 和 ASR Revision。
- ASR 文本冲突可见、可解释，规则版本写入合并 Receipt。
- 合并 Markdown 可从 Catalog 和来源关系重新生成。
- 删除或撤回合并 Revision 不会删除原始来源，且下游能看到 Evidence 状态变化。

## 与 V0.2 的关系

V0.2 先完成单 Chunk 的真实本地 ASR、模型切换、Receipt、Revision、时间范围和 Evidence Contract。V0.3 只在这些不可变来源和 provenance 能力之上增加合并层，不改变原始 ASR 的可信边界。
