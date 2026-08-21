//! Quick input: Fn-key push-to-talk → polish → paste at cursor.
//!
//! Registers a global hotkey (default `CommandOrControl+Shift+Space`) that
//! captures the current ASR time window, polishes it with the local LLM, and
//! pastes the result at the frontmost app's cursor. The global recording
//! stream is never interrupted; this module only slices the [t_start, t_end]
//! window from the already-running stream.

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Emitted when the hotkey is pressed (recording window starts).
pub const QUICK_INPUT_STARTED: &str = "quick-input-started";
/// Emitted when the hotkey is released (window closes, polish+paste begins).
pub const QUICK_INPUT_STOPPED: &str = "quick-input-stopped";
/// Emitted after the polished text has been pasted.
pub const QUICK_INPUT_POLISHED: &str = "quick-input-polished";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickInputState {
    pub active: bool,
    pub started_at_ms: i64,
}

/// Shared quick-input state owned by `AppState`.
pub struct QuickInput {
    active: AtomicBool,
    started_at_ms: AtomicI64,
}

impl Default for QuickInput {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            started_at_ms: AtomicI64::new(0),
        }
    }
}

impl QuickInput {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub fn mark_started(&self, now_ms: i64) {
        self.active.store(true, Ordering::SeqCst);
        self.started_at_ms.store(now_ms, Ordering::SeqCst);
    }

    pub fn mark_stopped(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn window(&self, now_ms: i64) -> (i64, i64) {
        let start = self.started_at_ms.load(Ordering::SeqCst);
        (start, now_ms)
    }
}

/// Registers the global hotkey and wires press/release to the quick-input flow.
pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let app_handle = app.clone();
    let shortcut = hotkey
        .parse::<Shortcut>()
        .map_err(|e| format!("invalid hotkey '{hotkey}': {e}"))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            let now_ms = now_millis();
            match event.state() {
                ShortcutState::Pressed => {
                    let state = app_handle.state::<crate::commands::AppState>();
                    state.quick_input.mark_started(now_ms);
                    let _ = app_handle.emit(
                        QUICK_INPUT_STARTED,
                        QuickInputState {
                            active: true,
                            started_at_ms: now_ms,
                        },
                    );
                }
                ShortcutState::Released => {
                    let state = app_handle.state::<crate::commands::AppState>();
                    let (start, end) = state.quick_input.window(now_ms);
                    state.quick_input.mark_stopped();
                    let _ = app_handle.emit(
                        QUICK_INPUT_STOPPED,
                        QuickInputState {
                            active: false,
                            started_at_ms: start,
                        },
                    );
                    // Trigger polish+paste asynchronously
                    let app_clone = app_handle.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = polish_and_paste(&app_clone, start, end) {
                            let _ = app_clone
                                .emit(QUICK_INPUT_POLISHED, serde_json::json!({ "error": e }));
                        }
                    });
                }
            }
        })
        .map_err(|e| format!("failed to register hotkey: {e}"))?;

    Ok(())
}

fn polish_and_paste(app: &AppHandle, start_ms: i64, end_ms: i64) -> Result<(), String> {
    // Extract the raw text from the streaming capture window.
    let raw_text = extract_window_text(app, start_ms, end_ms);

    // Polish via the local LLM.
    let frontmost = get_frontmost_app();
    let context = crate::llm::PolishContext {
        app_bundle_id: frontmost.clone(),
        preserve_raw: is_code_app(&frontmost),
    };
    let result = crate::llm::polish(&raw_text, "qwen2.5:0.5b", &context);

    // Paste at cursor.
    paste_text_at_cursor(&result.polished)?;

    let _ = app.emit(
        QUICK_INPUT_POLISHED,
        serde_json::json!({
            "polished": result.polished,
            "appBundleId": frontmost,
        }),
    );
    Ok(())
}

/// Extracts the raw ASR text within [start_ms, end_ms].
/// Production reads from the streaming capture buffer; development returns a
/// placeholder since the mock source emits segments directly to the WebView.
fn extract_window_text(_app: &AppHandle, _start_ms: i64, _end_ms: i64) -> String {
    // TODO(production): query the StreamingCapture's retained segment buffer.
    String::new()
}

/// Returns the frontmost app's bundle identifier via AppleScript.
pub fn get_frontmost_app() -> Option<String> {
    let output = Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get bundle identifier of first process whose frontmost is true"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Pastes text at the current cursor via System Events keystroke.
pub fn paste_text_at_cursor(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    // Escape for AppleScript string literal.
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"System Events\" to keystroke \"{escaped}\"");
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("osascript not found: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("paste failed: {stderr}"));
    }
    Ok(())
}

fn is_code_app(bundle_id: &Option<String>) -> bool {
    matches!(
        bundle_id.as_deref(),
        Some("com.microsoft.VSCode")
            | Some("com.apple.Terminal")
            | Some("com.googlecode.iterm2")
            | Some("com.jetbrains.intellij")
    )
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_input_tracks_active_window() {
        let qi = QuickInput::default();
        assert!(!qi.is_active());
        qi.mark_started(1000);
        assert!(qi.is_active());
        assert_eq!(qi.window(5000), (1000, 5000));
        qi.mark_stopped();
        assert!(!qi.is_active());
    }

    #[test]
    fn code_app_detection() {
        assert!(is_code_app(&Some("com.microsoft.VSCode".into())));
        assert!(is_code_app(&Some("com.apple.Terminal".into())));
        assert!(!is_code_app(&Some("com.apple.mail".into())));
        assert!(!is_code_app(&None));
    }

    #[test]
    fn paste_empty_text_is_noop() {
        assert!(paste_text_at_cursor("").is_ok());
    }
}
