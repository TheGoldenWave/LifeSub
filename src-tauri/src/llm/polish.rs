//! Local LLM polish service for ASR post-processing.
//!
//! Takes raw ASR transcript text and produces a polished version:
//! removes filler words, corrects self-corrections, and formats into
//! structured text. Production uses a local LLM (ollama / llama.cpp) and
//! must surface provider failures instead of silently returning demo output.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Result of a polish operation.
#[derive(Clone, Debug)]
pub struct PolishResult {
    pub original: String,
    pub polished: String,
    pub provider: String,
    pub model: String,
    pub fallback: Option<String>,
    pub error: Option<String>,
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

const PROVIDER_OLLAMA: &str = "ollama";
#[cfg(test)]
const PROVIDER_MOCK: &str = "mock";
#[cfg(test)]
const FALLBACK_MOCK_POLISH: &str = "mock_polish";
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const OLLAMA_TIMEOUT: Duration = Duration::from_secs(10);

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
pub fn polish_with_ollama(text: &str, model: &str, context: &PolishContext) -> PolishResult {
    let prompt = if context.preserve_raw {
        POLISH_PRESERVE_PROMPT
    } else {
        POLISH_SYSTEM_PROMPT
    };
    let full_prompt = format!("{prompt}{text}");

    match call_ollama(model, &full_prompt) {
        Ok(polished) => success_result(text, polished, PROVIDER_OLLAMA, model),
        Err(error) => failure_result(text, model, error),
    }
}

fn call_ollama(model: &str, prompt: &str) -> Result<String, String> {
    let (raw, stderr) = run_command_with_timeout(
        "ollama",
        &["run", model, "--format", "json"],
        prompt,
        OLLAMA_TIMEOUT,
    )?;

    let polished = raw
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            v.get("response")?.as_str().map(|s| s.to_string())
        })
        .collect::<Vec<_>>()
        .join("");

    if polished.is_empty() {
        if stderr.trim().is_empty() {
            return Err("ollama returned empty response".to_string());
        }
        return Err(format!("ollama returned empty response: {stderr}"));
    }

    Ok(polished.trim().to_string())
}

fn run_command_with_timeout(
    program: &str,
    args: &[&str],
    prompt: &str,
    timeout: Duration,
) -> Result<(String, String), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program} not found: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{program} stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("{program} stderr unavailable"))?;

    let stdout_reader = thread::spawn(move || read_pipe(stdout));
    let stderr_reader = thread::spawn(move || read_pipe(stderr));

    if let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(prompt.as_bytes())
    {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(format!("failed to write to {program}: {error}"));
    }

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    child
                        .wait()
                        .map_err(|e| format!("{program} failed to stop after timeout: {e}"))?;
                    let _ = stdout_reader.join();
                    let stderr = stderr_reader
                        .join()
                        .map_err(|_| format!("{program} stderr reader panicked"))??;
                    if stderr.is_empty() {
                        return Err(format!(
                            "{program} timed out after {} ms",
                            timeout.as_millis()
                        ));
                    }
                    return Err(format!(
                        "{program} timed out after {} ms: {stderr}",
                        timeout.as_millis()
                    ));
                }
                thread::sleep(COMMAND_POLL_INTERVAL);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("{program} status check failed: {error}"));
            }
        }
    }

    let output = child.wait().map_err(|e| format!("{program} failed: {e}"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| format!("{program} stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("{program} stderr reader panicked"))??;

    if !output.success() {
        return Err(format!("{program} error: {stderr}"));
    }

    Ok((stdout, stderr))
}

fn read_pipe<R: Read>(mut reader: R) -> Result<String, String> {
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .map_err(|error| format!("pipe read failed: {error}"))?;
    Ok(String::from_utf8_lossy(&buffer).trim().to_string())
}

/// Runs the polish operation with a timeout.
pub fn polish(text: &str, model: &str, context: &PolishContext) -> PolishResult {
    if text.trim().is_empty() {
        return success_result(text, String::new(), PROVIDER_OLLAMA, model);
    }

    let result = polish_with_ollama(text, model, context);
    if result.error.is_none() {
        if result.polished.trim().is_empty() {
            return failure_result(text, model, "ollama returned empty response".to_string());
        }
        return result;
    }

    result
}

fn success_result(text: &str, polished: String, provider: &str, model: &str) -> PolishResult {
    PolishResult {
        original: text.to_string(),
        polished,
        provider: provider.to_string(),
        model: model.to_string(),
        fallback: None,
        error: None,
    }
}

fn failure_result(text: &str, model: &str, error: String) -> PolishResult {
    PolishResult {
        original: text.to_string(),
        polished: String::new(),
        provider: PROVIDER_OLLAMA.to_string(),
        model: model.to_string(),
        fallback: None,
        error: Some(error),
    }
}

#[cfg(test)]
fn fallback_result(text: &str, model: &str, error: String) -> PolishResult {
    PolishResult {
        original: text.to_string(),
        polished: mock_polish(text),
        provider: PROVIDER_MOCK.to_string(),
        model: model.to_string(),
        fallback: Some(FALLBACK_MOCK_POLISH.to_string()),
        error: Some(error),
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
        assert!(result.error.is_none());
    }

    #[test]
    fn strict_mode_returns_empty_text_when_provider_fails() {
        let result = polish(
            "呃 这个 需要严格失败",
            "__definitely_missing_model__",
            &PolishContext::default(),
        );

        assert_eq!(result.polished, "");
        assert_eq!(result.provider, PROVIDER_OLLAMA);
        assert_eq!(result.model, "__definitely_missing_model__");
        assert!(result.fallback.is_none());
        assert!(result.error.is_some());
    }

    #[test]
    fn fallback_result_marks_mock_provenance() {
        let result = fallback_result("呃 这个 需要演示回退", "demo-model", "forced".to_string());
        assert!(!result.polished.is_empty());
        assert_eq!(result.provider, PROVIDER_MOCK);
        assert_eq!(result.model, "demo-model");
        assert_eq!(result.fallback.as_deref(), Some(FALLBACK_MOCK_POLISH));
        assert!(result.error.is_some());
    }

    #[test]
    fn command_timeout_returns_error() {
        let result =
            run_command_with_timeout("sh", &["-c", "sleep 1"], "", Duration::from_millis(10));

        let error = result.expect_err("timeout expected");
        assert!(error.contains("timed out"));
    }

    #[test]
    fn command_reads_stdout_and_stderr_without_blocking() {
        let result = run_command_with_timeout(
            "sh",
            &["-c", "printf '{\"response\":\"ok\"}\\n'; printf 'warn' >&2"],
            "",
            Duration::from_secs(1),
        )
        .expect("command should succeed");

        assert_eq!(result.0, "{\"response\":\"ok\"}");
        assert_eq!(result.1, "warn");
    }
}
