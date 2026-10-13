pub mod api;
pub mod asr;
pub mod capture;
pub mod catalog;
#[cfg(feature = "desktop")]
pub mod desktop_runtime;
pub mod diagnostics;
pub mod domain;
pub mod llm;
#[cfg(feature = "desktop")]
pub mod quick_input;
pub mod service;

#[cfg(feature = "desktop")]
mod commands;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(test)]
mod asr_audio_test;
#[cfg(test)]
mod asr_job_snapshot_test;
#[cfg(test)]
mod asr_job_test;
#[cfg(test)]
mod asr_manifest_test;
#[cfg(test)]
mod asr_model_manager_test;
#[cfg(test)]
mod asr_native_engine_test;
#[cfg(test)]
mod asr_provider_test;
#[cfg(test)]
mod asr_runtime_qualifier_test;
#[cfg(test)]
mod asr_runtime_test;
#[cfg(test)]
mod asr_service_test;
#[cfg(test)]
mod asr_settings_test;
#[cfg(test)]
mod asr_vad_test;
#[cfg(test)]
mod capture_coordinator_test;
#[cfg(test)]
mod capture_helper_test;
#[cfg(test)]
mod capture_protocol_test;
#[cfg(test)]
mod capture_seal_test;
#[cfg(test)]
mod catalog_migration_test;
#[cfg(test)]
mod catalog_test;
#[cfg(test)]
mod diagnostics_test;
#[cfg(test)]
mod domain_test;
#[cfg(test)]
mod service_test;
#[cfg(all(test, feature = "desktop"))]
mod session_test;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            match commands::startup_disposition(commands::AppState::initialize(app.handle())) {
                commands::StartupDisposition::Ready(state) => {
                    app.manage(state);
                }
                commands::StartupDisposition::Exit { reason } => {
                    eprintln!("LifeSub startup stopped: {reason}");
                    app.handle().exit(1);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_capture_session,
            commands::transition_capture_session,
            commands::import_audio_file,
            commands::import_audio_record,
            commands::create_manual_revision,
            commands::list_timeline_records,
            commands::get_asr_session_state,
            commands::search_transcripts,
            commands::resolve_evidence,
            // Task 13.5
            commands::create_note,
            commands::list_notes,
            commands::update_note,
            commands::delete_note,
            commands::create_category,
            commands::list_categories,
            commands::delete_category,
            commands::create_entry,
            commands::list_entries,
            commands::update_entry,
            commands::toggle_entry,
            commands::delete_entry,
            commands::list_voiceprints,
            commands::register_voiceprint,
            commands::rename_voiceprint,
            commands::delete_voiceprint,
            commands::link_voiceprint_to_entry,
            commands::get_stats_snapshot,
            commands::get_asr_config,
            commands::get_settings_capabilities,
            commands::set_asr_config,
            commands::get_recording_config,
            commands::set_recording_config,
            commands::list_microphone_devices,
            commands::probe_microphone,
            commands::get_diagnostics,
            commands::write_diagnostics_export,
            commands::write_record_markdown_export,
            commands::open_macos_privacy_settings,
            commands::get_app_runtime_info,
            commands::list_asr_models,
            commands::install_asr_model,
            commands::cancel_model_download,
            commands::retry_model_download,
            // Phase 2.1 streaming
            commands::start_streaming_capture,
            commands::stop_streaming_capture,
            commands::pause_streaming_capture,
            commands::resume_streaming_capture,
            // Phase 3: LLM polish + quick input
            commands::llm_polish,
            commands::register_quick_input_hotkey,
            commands::get_frontmost_app,
            commands::paste_text_at_cursor,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LifeSub desktop app");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
