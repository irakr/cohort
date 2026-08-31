//! Cohort app shell. Hosts the webview UI and the owner agent module
//! (cohort-agent). The agent commands run locally; their output reaches the
//! hub only when the owner explicitly shares it.

use cohort_agent::{AgentModule, ArtifactGroup, LocalAgent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

#[tauri::command]
fn suggest_artifacts() -> Vec<ArtifactGroup> {
    let groups = LocalAgent.suggest_artifacts();
    log::info!(
        "artifact scan: {} group(s), {} item(s)",
        groups.len(),
        groups.iter().map(|g| g.items.len()).sum::<usize>()
    );
    groups
}

#[tauri::command]
fn env_fingerprint() -> Vec<String> {
    LocalAgent.env_fingerprint()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // All app logs (Rust `log` macros and forwarded webview console
            // output) go to the shared cohort namespace (see cohort-dirs):
            //   macOS   ~/Library/Application Support/cohort/logs/app.log
            //   Linux   ~/.config/cohort/logs/app.log
            //   Windows %APPDATA%\cohort\logs\app.log
            let log_dir = cohort_dirs::logs_dir().expect("no OS config directory");
            app.handle().plugin(
                tauri_plugin_log::Builder::new()
                    .level(log::LevelFilter::Info)
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::Folder {
                            path: log_dir.clone(),
                            file_name: Some("app".into()),
                        }),
                    ])
                    .max_file_size(2 * 1024 * 1024)
                    .rotation_strategy(RotationStrategy::KeepOne)
                    .build(),
            )?;
            log::info!("cohort app started; logging to {}", log_dir.join("app.log").display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![suggest_artifacts, env_fingerprint])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
