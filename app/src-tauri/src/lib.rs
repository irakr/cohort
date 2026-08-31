//! Cohort app shell. Hosts the webview UI and the owner agent module
//! (cohort-agent). The agent commands run locally; their output reaches the
//! hub only when the owner explicitly shares it from the context picker.

use cohort_agent::{AgentModule, ArtifactGroup, LocalAgent};

#[tauri::command]
fn suggest_artifacts() -> Vec<ArtifactGroup> {
    LocalAgent.suggest_artifacts()
}

#[tauri::command]
fn env_fingerprint() -> Vec<String> {
    LocalAgent.env_fingerprint()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![suggest_artifacts, env_fingerprint])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
