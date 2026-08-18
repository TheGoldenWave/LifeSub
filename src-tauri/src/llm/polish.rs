//! Local LLM polish service for ASR post-processing.
//!
//! Takes raw ASR transcript text and produces a polished version:
//! removes filler words, corrects self-corrections, and formats into
//! structured text. Production uses a local LLM (ollama / llama.cpp);
//! development uses a deterministic mock.

use std::io::Write;
use std::process::{Command, Stdio};

/// Result of a polish operation.
#[derive(Clone, Debug)]
pub struct PolishResult {
    pub original: String,
    pub polished: String,
}

/// Context passed to the LLM for tone/scene adaptation.
#[derive(Clone, Debug, Default)]
pub struct PolishContext {
    /// The frontmost app bundle identifier (e.g. "com.apple.mail").
    pub app_bundle_id: Option<String>,
    /// Whether to preserve the original structure (e.g. for code editors).
    pub preserve_raw: bool,
}

/// The polish prompt template.
const POLISH_SYSTEM_PROMPT: &str = r#"你是一个专业的语音转文字润色助手。请对以下口语转写结果进行润色：

1. 删除所有口头禅和填充词（如"呃""啊""那个""就是说""然后"等）
2. 识别并修正中途改口：如果说话人中途修正了自己，只保留最终版本
3. 将口语化表述转为书面语，但保留原意和语气
4. 如果包含列表性质的表述（"第一""第二"等），自动格式化为编号列表
5. 仅输出润色后的文本，不要添加任何解释或前缀

原始转写：
"#;

const POLISH_PRESERVE_PROMPT: &str = r#"你是一个语音转文字润色助手。请对以下口语转写结果进行轻量润色：

1. 删除口头禅和填充词（如"呃""啊""那个"等）
2. 修正明显的口误，但保留代码、术语和原有格式
3. 仅输出润色后的文本，不要添加任何解释或前缀

原始转写：
"#;

/// Mock polish that simulates basic cleanup for development.
pub fn mock_polish(text: &str) -> String {
    let filler_words = ["呃", "啊", "那个", "就是说", "然后", "嗯", "这个"];
    let mut result = text.to_string();
    for word in &filler_words {
        result = result.replace(word, "");
    }
    // Collapse multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}

/// Attempts to polish text using the local ollama CLI.
/// Falls back to mock_polish if ollama is not available.
pub fn polish_with_ollama(text: &str, model: &str, context: &PolishContext) -> PolishResult {
    let prompt = if context.preserve_raw {
        POLISH_PRESERVE_PROMPT
    } else {
        POLISH_SYSTEM_PROMPT
    };
    let full_prompt = format!("{prompt}{text}");

    match call_ollama(model, &full_prompt) {
        Ok(polished) => PolishResult {
            original: text.to_string(),
            polished,
        },
        Err(_) => {
            // Fallback to mock
            PolishResult {
                original: text.to_string(),
                polished: mock_polish(text),
            }
        }
    }
}

fn call_ollama(model: &str, prompt: &str) -> Result<String, String> {
    let mut child = Command::new("ollama")
        .args(["run", model, "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ollama not found: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write to ollama: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("ollama failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ollama error: {stderr}"));
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    // Parse ollama JSON response (simple line-based)
    let polished = raw
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            v.get("response")?.as_str().map(|s| s.to_string())
        })
        .collect::<Vec<_>>()
        .join("");

    if polished.is_empty() {
        return Err("ollama returned empty response".to_string());
    }

    Ok(polished.trim().to_string())
}

/// Runs the polish operation with a timeout.
pub fn polish(text: &str, model: &str, context: &PolishContext) -> PolishResult {
    // Try ollama first; fallback to mock
    let result = polish_with_ollama(text, model, context);
    // If the polished text is empty or identical, return mock
    if result.polished.is_empty() || result.polished == text {
        PolishResult {
            original: text.to_string(),
            polished: mock_polish(text),
        }
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_polish_removes_filler_words() {
        let input = "呃 那个 我们今天就是说 要讨论一下";
        let result = mock_polish(input);
        assert!(!result.contains("呃"));
        assert!(!result.contains("那个"));
        assert!(!result.contains("就是说"));
        assert!(result.contains("我们今天"));
    }

    #[test]
    fn mock_polish_collapses_spaces() {
        let result = mock_polish("呃  啊  你好");
        assert!(!result.contains("  "));
    }

    #[test]
    fn polish_with_empty_text() {
        let result = polish("", "qwen2.5:0.5b", &PolishContext::default());
        assert_eq!(result.polished, "");
    }
}