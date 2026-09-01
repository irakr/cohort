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

/// Owner side: what is running in the granted terminals right now (process
/// list per tty). Published to the hub while the assist is open; real PTY
/// streaming arrives with the detector.
#[tauri::command]
fn terminal_activity(labels: Vec<String>) -> Vec<String> {
    cohort_agent::scan::terminal_activity(&labels)
}

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default()
}

/// This machine's SSH public key (travels with an ssh access request).
#[tauri::command]
fn ssh_public_key() -> Option<String> {
    cohort_agent::ssh::public_key(&home())
}

/// Suggested user@host for granting SSH access to this machine.
#[tauri::command]
fn ssh_target_suggestion() -> String {
    cohort_agent::ssh::target_suggestion()
}

/// Owner side: install a responder's public key (tagged with the marker)
/// into authorized_keys. Runs only on explicit grant approval.
#[tauri::command]
fn install_ssh_key(public_key: String, marker: String) -> Result<(), String> {
    cohort_agent::ssh::install_key(&home(), &public_key, &marker).map_err(|e| e.to_string())?;
    log::info!("installed ssh key for grant {marker}");
    Ok(())
}

/// Responder side: open the system terminal running ssh to a granted target.
#[tauri::command]
fn open_ssh(target: String) -> Result<(), String> {
    log::info!("opening ssh session to {target}");
    cohort_agent::ssh::open_terminal_ssh(&target)
}

/// Snapshot shared files/directories (bounded, redacted) for upload to the
/// hub as the assist's live data. Runs only on explicit share or grant.
#[tauri::command]
fn snapshot_artifacts(paths: Vec<String>) -> cohort_agent::snapshot::PathSnapshot {
    let snap = cohort_agent::snapshot::snapshot_paths(&paths);
    log::info!(
        "snapshot: {} path(s) -> {} file(s), {} note(s)",
        paths.len(),
        snap.files.len(),
        snap.notes.len()
    );
    snap
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
        .invoke_handler(tauri::generate_handler![
            suggest_artifacts,
            env_fingerprint,
            snapshot_artifacts,
            terminal_activity,
            ssh_public_key,
            ssh_target_suggestion,
            install_ssh_key,
            open_ssh
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
