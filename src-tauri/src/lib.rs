pub mod asr;
pub mod catalog;
pub mod domain;
pub mod service;

#[cfg(feature = "desktop")]
mod commands;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(test)]
mod asr_audio_test;
#[cfg(test)]
mod asr_manifest_test;
#[cfg(test)]
mod asr_model_manager_test;
#[cfg(test)]
mod asr_runtime_test;
#[cfg(test)]
mod asr_settings_test;
#[cfg(test)]
mod asr_vad_test;
#[cfg(test)]
mod catalog_migration_test;
#[cfg(test)]
mod catalog_test;
#[cfg(test)]
mod domain_test;
#[cfg(test)]
mod service_test;

#[cfg(feature = "desktop")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(commands::AppState::initialize(app.handle())?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_capture_session,
            commands::transition_capture_session,
            commands::import_audio_file,
            commands::append_transcript_revision,
            commands::search_transcripts,
            commands::resolve_evidence,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LifeSub desktop app");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
