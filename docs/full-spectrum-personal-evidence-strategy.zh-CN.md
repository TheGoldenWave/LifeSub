# LifeSub 全领域个人 Evidence 长期战略

状态：已确认长期产品方向，不改变当前版本范围

日期：2026-08-28

## 1. 决策摘要

LifeSub 的当前实现楔子继续是 macOS 长时音频、声纹、ASR、Transcript Revision 和 Evidence Ref；长期定位升级为：

> **LifeSub 是本地优先、跨设备、跨模态的个人 Evidence 基础设施，统一记录人经历了什么、设备感知到了什么、外部系统记录了什么。**

音频不是最终品类边界，而是最先验证、价值密度高且 Evidence 语义最清楚的起点。图片、视频、屏幕、智能硬件、健康传感器和数字活动记录后续通过 Source Adapter 接入同一 Evidence Core，不分别建立平行事实源。

## 2. 双层产品定位

| 层级 | 定位 | 当前承诺 |
|---|---|---|
| 当前实现楔子 | 长时音频与 ASR Evidence | 原始音频、Chunk、ASR、声纹、Revision、Catalog、Evidence Ref |
| 长期产品方向 | 全领域个人 Evidence Fabric | 多模态采集、跨设备时间线、来源证明、权限、撤回和统一引用 |

长期方向不能用于提前宣称视觉、传感器或健康数据已经实现，也不能阻塞当前真实录音和 native ASR 主链路。

## 3. Evidence 范围

| Evidence 类型 | 示例 | 典型派生处理 |
|---|---|---|
| 声音 | 麦克风、系统音频、会议、环境声音 | ASR、声纹、说话人分离、事件检测 |
| 视觉 | 图片、视频、眼镜抓拍、屏幕记录 | OCR、多模态描述、对象与场景索引 |
| 生理传感器 | 心率、睡眠、血氧、体温、活动量 | 时间窗聚合、趋势与异常检测 |
| 环境传感器 | 位置、噪声、光线、运动、设备状态 | 时间线对齐、场景标签 |
| 数字活动 | 日历、通信记录、浏览与应用事件 | 事件归一化、来源引用、状态变化 |
| 行动回执 | 预约、支付、提交、部署等外部结果 | 结果校验、状态变化、Outcome Evidence |

LifeSub 是 Evidence 权威，不是“保存一切”的个人数据湖。任何接入都必须能回答：来源是什么、何时产生、原件在哪里、经过了哪些处理、谁可读取、能否撤回。

对于 Apple Health、日历、通信平台等外部系统，源平台仍是 source-of-origin；LifeSub 拥有的是获授权采集后的个人 Evidence 记录、来源证明、时间线、派生版本和稳定引用，不伪装成外部系统的实时业务权威。

## 4. 架构边界

```text
Audio / Camera / Wearable / Health API / Calendar / App Event
                              ↓
                    LifeSub Source Adapters
                              ↓
                     LifeSub Evidence Core
       Raw + Time + Source + Hash + Permission + Revision
                              ↓
                    lifesub:// Evidence Ref
                    ↓                       ↓
                  Malow                 GoldenWave
          Decision / Action / Review   Governed Memory
```

- Source Adapter 负责设备、平台和格式适配，不拥有独立 Evidence 语义。
- Evidence Core 负责不可变原件、派生版本、时间、来源、授权、保留、撤回和稳定引用。
- ASR、OCR、多模态理解和传感器聚合是 Derived Evidence，不能覆盖或伪装成原始数据。
- 插件和宿主只消费 Contract，不复制 LifeSub 数据库或重建权限判断。

## 5. Observation、Interpretation、Action、Memory

四类对象必须分开：

| 对象 | 示例 | 权威系统 |
|---|---|---|
| Observation | 手表记录过去三晚平均睡眠 5 小时 | LifeSub |
| Derived Evidence | 可追溯规则判断睡眠时长持续偏低 | LifeSub，可重建派生物 |
| Decision / Action | 建议停止加班、预约医生并执行 | Malow |
| Long-term Fact / Experience | 经确认的长期睡眠事实或个人经验 | GoldenWave |

LifeSub 可以保存“测到了什么”和“如何派生”，但不负责医疗诊断、人生优先级、项目解释或自动行动。Malow 执行动作后，外部回执可以再次成为 LifeSub Evidence；GoldenWave 只接收带来源、经过治理的长期 Candidate。

## 6. 与四项目体系的关系

- **LifeSub = Evidence Fabric**：个人现实、设备世界和数字活动的可追溯证据。
- **Follow-up = Signal Fabric**：外部世界可能值得注意的策展信号。
- **Malow = Work Control Plane**：基于 Evidence 和 Signal 做决策、执行、Review 与 Outcome。
- **GoldenWave = Memory Governance**：治理长期事实、知识、经验和 Capability 状态。

## 7. 演进 Gate

1. 先完成真实音频采集、native ASR、Revision 和 Evidence Ref 闭环。
2. 验证跨设备音频与已有录音设备导入。
3. 只在真实缺口证明必要时增加图片、屏幕或低频视频 Evidence。
4. 通过 Apple Health 等窄接口验证传感器时间序列、权限和撤回语义。
5. 再评估环境传感器、数字活动和更多硬件 Producer。
6. 每种新来源独立通过隐私、保留、删除、来源真实性和下游撤回 Gate。

新来源接入不得以“模型能理解”为完成标准，必须先证明 Evidence Contract、原件可追溯和权限可预测。
