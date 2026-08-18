# Task 13.5 — 后端模块补全清单

> **父分支**: `codex/lifesub-real-asr-v0.2`
> **前置**: Task 13 (mutation 方法 + idempotency + protocol)
> **后置**: 新 UI 页面接入真实后端
> **状态**: 待排期

---

## 总览

前端 UI 已重构为 4 页面架构（录音 / 时间线 / 词典 / 设置弹窗），但当前所有数据均来自 `src/data/demo.ts` 的静态 demo 数据。后端只有 6 个 capture session 生命周期命令，不覆盖新 UI 引入的任何功能模块。

本 Task 记录需要新增的全部后端模块，按优先级分 4 组。

### 核心 Pipeline 架构

```
                      原始音频
                         │
         ┌───────────────┼───────────────┐
         │               │               │
    ASR 转写         说话人分离        声纹识别
 ┌──────────┐    ┌──────────────┐    ┌──────────┐
 │SenseVoice│    │sherpa-onnx   │    │  CAM++   │
 │Whisper   │    │Speaker Seg   │    │embedding │
 │Qwen3-ASR*│    │   (独立模型)   │    │  + 比对   │
 └────┬─────┘    └──────┬───────┘    └────┬─────┘
      │                 │                 │
      │          匿名 spk 切分         声纹向量
      │     [(spk_0,0~12s),...]    vs 声纹库
      │                 │                 │
      │     ┌───────────┴──────────┐      │
      │     │   > 阈值 → "张伟"      │      │
      │     │   ≤ 阈值 → "未知说话人"   │      │
      │     └───────────┬──────────┘      │
      │                 │                 │
      └─────────┬───────┴─────────────────┘
                │
       时间戳对齐 → 标注段落
 ┌──────────────┼──────────────┐
 │              │              │
[00:00] 张伟  [00:12] 我   [00:25] 未知说话人 1
```

> \* Qwen3-ASR 自带 Diarization（speaker 字段），选它时跳过 sherpa-onnx Diarization，直接复用其 spk 切分。CAM++ 声纹匹配逻辑不变。

---

## Group A: 笔记 CRUD（阻塞录音页）

> 优先级：P0 — 录音页的笔记功能完全无法落地

### 数据模型

```rust
// 笔记标签
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteTag {
    Todo,       // 待办
    Memo,       // 备忘
    Question,   // 问题
    Decision,   // 决定
    Custom(String),
}

// 笔记记录
pub struct CaptureNote {
    pub id: String,
    pub session_id: String,
    pub content: String,
    pub timestamp_ms: i64,
    pub tag: NoteTag,
    pub segment_id: Option<String>,
    pub created_at: String,
}
```

### Catalog 迁移

| 表 | 关键列 |
|----|--------|
| `notes` | `id TEXT PK`, `session_id TEXT FK→sessions`, `content TEXT`, `timestamp_ms INTEGER`, `tag TEXT`, `segment_id TEXT?`, `created_at TEXT` |

### Tauri 命令

| 命令 | 签名 |
|------|------|
| `create_note` | `(session_id: String, content: String, timestamp_ms: i64, tag: String, segment_id: Option<String>) → CaptureNote` |
| `list_notes` | `(session_id: String) → Vec<CaptureNote>` |
| `delete_note` | `(note_id: String) → ()` |
| `update_note` | `(note_id: String, content: String, tag: String) → CaptureNote` |

### 前端 invoke wrapper

`src/services/lifesub.ts` 新增：`createNote`, `listNotes`, `deleteNote`, `updateNote`

---

## Group B: 词典 CRUD（阻塞词典页）

> 优先级：P0 — 词典页的增删改查完全无法落地

### 数据模型

```rust
pub struct DictionaryCategory {
    pub id: String,
    pub name: String,
    pub scope: String,       // "global" | "project:{id}"
    pub entry_count: i32,
}

pub struct DictionaryEntry {
    pub id: String,
    pub category_id: String,
    pub term: String,
    pub pinyin: String,
    pub aliases: String,     // 分号分隔
    pub note: String,
    pub enabled: bool,
}
```

### Catalog 迁移

| 表 | 关键列 |
|----|--------|
| `dictionary_categories` | `id TEXT PK`, `name TEXT`, `scope TEXT`, `entry_count INTEGER` |
| `dictionary_entries` | `id TEXT PK`, `category_id TEXT FK`, `term TEXT`, `pinyin TEXT`, `aliases TEXT`, `note TEXT`, `enabled INTEGER` |

### Tauri 命令

| 命令 | 签名 |
|------|------|
| `list_dictionary_categories` | `(scope: Option<String>) → Vec<DictionaryCategory>` |
| `create_category` | `(name: String, scope: String) → DictionaryCategory` |
| `delete_category` | `(category_id: String) → ()` |
| `list_entries` | `(category_id: String, query: Option<String>) → Vec<DictionaryEntry>` |
| `create_entry` | `(category_id: String, term: String, pinyin: String, aliases: String, note: String) → DictionaryEntry` |
| `update_entry` | `(entry_id: String, ...) → DictionaryEntry` |
| `delete_entry` | `(entry_id: String) → ()` |
| `toggle_entry` | `(entry_id: String, enabled: bool) → DictionaryEntry` |

### 前端 invoke wrapper

`src/services/lifesub.ts` 新增：`listCategories`, `createCategory`, `deleteCategory`, `listEntries`, `createEntry`, `updateEntry`, `deleteEntry`, `toggleEntry`

---

## Group C: 声纹库（阻塞说话人自动识别）

> 优先级：P1 — 录音页的声纹标注功能无法落地，但可先用手动标注
>
> **声纹引擎选型：FunASR CAM++**
>
> 三者对比：
> - **sherpa-onnx Speaker Recognition**：有模型但 WASM SDK 的 embedding 提取仍不稳定（[issue #1979](https://github.com/k2-fsa/sherpa-onnx/issues/1979)），不适合跨会话持久化声纹库。
> - **Qwen3-ASR**：自带说话人分离字段（spk_0/spk_1），但仅限**单次转录内**标注，不产出可跨会话复用的声纹向量，无独立 embedding API。
> - **FunASR CAM++**：[CAM++](https://huggingface.co/funasr/campplus) 是专门做 speaker embedding 的轻量模型（~10MB），提取的向量可持久化到声纹库，跨会话比对。与 sherpa-onnx 不冲突 — sherpa 管 ASR，CAM++ 管"谁说的"。
>
> 架构：
> ```
>                       原始音频
>                          │
>          ┌───────────────┼───────────────┐
>          │               │               │
>     ASR 转写         说话人分离        声纹识别
>  ┌──────────┐    ┌──────────────┐    ┌──────────┐
>  │SenseVoice│    │sherpa-onnx   │    │  CAM++   │
>  │Whisper   │    │Speaker Seg   │    │embedding │
>  │Qwen3-ASR │    │   (独立模型)   │    │  + 比对   │
>  └────┬─────┘    └──────┬───────┘    └────┬─────┘
>       │                 │                 │
>       │           匿名 spk 切分        声纹向量
>       │      [(spk_0,0~12s),...]    vs 声纹库
>       │                 │                 │
>       │      ┌──────────┴──────────┐      │
>       │      │   > 阈值 → "张伟"     │      │
>       │      │   ≤ 阈值 → "未知说话人" │      │
>       │      └──────────┬──────────┘      │
>       │                 │                 │
>       └─────────┬───────┴─────────────────┘
>                 │
>        时间戳对齐 → 标注段落
>  ┌──────────────┼──────────────┐
>  │              │              │
> [00:00] 张伟  [00:12] 我   [00:25] 未知说话人 1
> ```
>
> **Qwen3-ASR 自带 Diarization 的变体路径**：
> ```
> Qwen3-ASR 一次推理同时产出：
>   ├── 转写文本 + 时间戳
>   └── speaker 标签 (spk_0/spk_1/spk_2)
>                        │
>              跳过 sherpa-onnx Diarization
>              直接拿 Qwen3 的 spk 切分
>                        │
>              对每个 spk_N 取音频片段
>                        │
>                  CAM++ 提取 embedding
>                        │
>                  声纹库比对 → 人名
> ```
>
> 两种路径的差异仅在"说话人切分"的来源，**CAM++ 声纹匹配和声纹库完全不变**。切换 ASR Provider 时，Diarization 来源自动切换：
>
> | ASR Provider | Diarization 来源 | 说明 |
> |-------------|-----------------|------|
> | SenseVoice | sherpa-onnx Speaker Segmentation | 独立 Diarization 模型 |
> | Whisper | sherpa-onnx Speaker Segmentation | Whisper 本身无 Diarization |
> | Qwen3-ASR | Qwen3 内置 speaker 字段 | 跳过 sherpa Diarization，省一次推理 |

### 数据模型

```rust
pub struct Voiceprint {
    pub id: String,
    pub name: String,
    pub embedding_path: String,      // 本地 .bin 文件路径（CAM++ embedding 向量）
    pub dictionary_entry_id: Option<String>,
    pub sample_count: i32,
    pub updated_at: String,
}
```

### Catalog 迁移

| 表 | 关键列 |
|----|--------|
| `voiceprints` | `id TEXT PK`, `name TEXT`, `embedding_path TEXT`, `dictionary_entry_id TEXT?`, `sample_count INTEGER`, `updated_at TEXT` |

### Tauri 命令

| 命令 | 签名 |
|------|------|
| `list_voiceprints` | `() → Vec<Voiceprint>` |
| `register_voiceprint` | `(name: String, audio_path: String) → Voiceprint` — 调 CAM++ 提取 embedding |
| `rename_voiceprint` | `(voiceprint_id: String, name: String) → Voiceprint` |
| `delete_voiceprint` | `(voiceprint_id: String) → ()` |
| `link_voiceprint_to_entry` | `(voiceprint_id: String, entry_id: String) → Voiceprint` |
| `match_voiceprint` | `(audio_path: String) → Option<Voiceprint>` — 与声纹库比对，返回最佳匹配 |

### 前端 invoke wrapper

`src/services/lifesub.ts` 新增：`listVoiceprints`, `registerVoiceprint`, `renameVoiceprint`, `deleteVoiceprint`, `linkVoiceprintToEntry`, `matchVoiceprint`

---

## Group D: 统计与设置（时间线统计条 + 设置弹窗）

> 优先级：P2 — 统计条可先展示 demo 数据，设置可先做纯前端

### D1: 24h 统计

| 命令 | 签名 |
|------|------|
| `get_stats_snapshot` | `(date: Option<String>) → StatsSnapshot` |

不需要新表，聚合查询 `sessions` 表即可。

### D2: ASR 设置

| 命令 | 签名 |
|------|------|
| `get_asr_config` | `() → AsrConfig` |
| `set_asr_config` | `(config: AsrConfig) → AsrConfig` |

```rust
pub struct AsrConfig {
    pub provider: String,          // "sensevoice" | "whisper" | "qwen3-asr"
    pub language: String,          // "zh" | "en" | "auto"
    pub auto_transcribe: bool,
    pub threads: i32,
    pub vad_enabled: bool,
    pub vad_min_speech_ms: i32,
    pub vad_silence_ms: i32,
    pub itn_enabled: bool,
}
```

存储方式：`settings` 表（key-value），或 `~/.lifesub/config.json`。

> **Provider 切换对 Diarization 的影响**：
> `provider` 字段驱动 Diarization 来源自动切换。后端在构建转录 pipeline 时根据此字段决定：
> - `sensevoice` / `whisper` → 使用 sherpa-onnx Speaker Segmentation 模型做 Diarization
> - `qwen3-asr` → 使用 Qwen3 内置 speaker 字段，跳过 sherpa Diarization
>
> 无论哪种路径，**CAM++ 声纹匹配和声纹库逻辑完全不变**。Provider 切换不影响声纹库的注册、比对和标注流程。

### D3: 录音设置

| 命令 | 签名 |
|------|------|
| `get_recording_config` | `() → RecordingConfig` |
| `set_recording_config` | `(config: RecordingConfig) → RecordingConfig` |

```rust
pub struct RecordingConfig {
    pub capture_mode: String,      // "smart" | "mic-only" | "system-only"
    pub im_detection_enabled: bool,
    pub im_apps: Vec<String>,      // ["wechat", "dingtalk", "feishu", ...]
    pub detection_delay_secs: i32, // 3
    pub recovery_delay_secs: i32,  // 5
    pub sample_rate: i32,          // 16000
    pub storage_path: String,      // "~/.lifesub/recordings/"
}
```

---

## 排期建议

| 组 | 优先级 | 阻塞页面 | 预估工作量 |
|----|--------|---------|-----------|
| A: 笔记 CRUD | P0 | 录音页 | 小（1 表 + 4 命令） |
| B: 词典 CRUD | P0 | 词典页 | 中（2 表 + 8 命令） |
| C: 声纹库 | P1 | 录音页声纹标注 | 中（1 表 + 5 命令，依赖 sherpa-onnx embedding） |
| D: 统计与设置 | P2 | 时间线条 + 设置弹窗 | 小（聚合查询 + 2 配置对象） |

推荐顺序：**A → B → D → C**（先把 P0 页面的 CRUD 串起来，再做声纹依赖项）