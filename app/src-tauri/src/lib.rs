//! Cohort app shell. Hosts the webview UI and the owner agent module
//! (cohort-agent). The agent commands run locally; their output reaches the
//! hub only when the owner explicitly shares it.

use cohort_agent::assistant::{self, DraftOutcome, InsightsInput, LlmConfig, Preset};
use cohort_agent::{AgentModule, ArtifactGroup, LocalAgent};
use tauri_plugin_log::{FileOpenStrategy, RotationStrategy, Target, TargetKind};

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

/// Owner side: capture one JPEG frame of a granted window, returned as
/// base64 (raw byte arrays are wasteful across the IPC boundary).
#[tauri::command]
fn capture_window(target: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = cohort_agent::windows::capture_target(&target)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
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

// ---- Assistant: the model runs from this machine, with this machine's
// settings. See cohort_agent::assistant.

#[tauri::command]
fn assistant_presets() -> Vec<Preset> {
    assistant::presets()
}

#[tauri::command]
fn assistant_config_get() -> Option<LlmConfig> {
    assistant::config::load()
}

/// `None` forgets the configuration; the assistant then reports itself as
/// not configured everywhere.
#[tauri::command]
fn assistant_config_set(config: Option<LlmConfig>) -> Result<(), String> {
    match config {
        Some(cfg) => {
            assistant::config::save(&cfg).map_err(|e| e.to_string())?;
            log::info!("assistant configured: {:?} {} {}", cfg.protocol, cfg.base_url, cfg.model);
        }
        None => {
            assistant::config::clear().map_err(|e| e.to_string())?;
            log::info!("assistant configuration removed");
        }
    }
    Ok(())
}

/// One tiny round trip with settings that may not be saved yet.
#[tauri::command]
async fn assistant_config_test(config: LlmConfig) -> Result<String, String> {
    assistant::test_config(&config).await
}

/// Owner side, when opening an assist: draft the insights from the title,
/// the description and the shared files, read here.
#[tauri::command]
async fn draft_insights(input: InsightsInput) -> DraftOutcome {
    let cfg = assistant::config::load();
    let outcome = assistant::draft_insights(cfg.as_ref(), &input).await;
    match (&outcome.note, &outcome.model) {
        (None, Some(model)) => log::info!(
            "insights drafted by {model}: {} tokens in, {} out",
            outcome.input_tokens, outcome.output_tokens
        ),
        (Some(note), _) => log::warn!("insights not drafted: {note}"),
        (None, None) => {}
    }
    outcome
}

/// Level for Cohort's own crates (the shell, cohort-agent, cohort-llm).
/// Debug in development builds, so the assistant's prompts and replies land
/// in app.log; info in release. `COHORT_LOG=error|warn|info|debug|trace`
/// overrides either. Third-party crates always stay at info.
fn cohort_log_level() -> log::LevelFilter {
    let default = if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    std::env::var("COHORT_LOG").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // All app logs (Rust `log` macros and forwarded webview console
            // output) go to the shared cohort namespace (see cohort-dirs):
            //   macOS   ~/Library/Application Support/cohort/logs/app.log
            //   Linux   ~/.config/cohort/logs/app.log
            //   Windows %APPDATA%\cohort\logs\app.log
            let log_dir = cohort_dirs::logs_dir().expect("no OS config directory");
            let cohort_level = cohort_log_level();
            app.handle().plugin(
                tauri_plugin_log::Builder::new()
                    .level(log::LevelFilter::Info)
                    .level_for("cohort_app_lib", cohort_level)
                    .level_for("cohort_agent", cohort_level)
                    .level_for("cohort_llm", cohort_level)
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::Folder {
                            path: log_dir.clone(),
                            file_name: Some("app".into()),
                        }),
                    ])
                    .max_file_size(2 * 1024 * 1024)
                    // Each run starts a fresh app.log: Rotate discards the
                    // previous session's file on open (KeepOne deletes rather
                    // than archives), so logs never span runs. Within a run
                    // the size cap still rotates.
                    .file_open_strategy(FileOpenStrategy::Rotate)
                    .rotation_strategy(RotationStrategy::KeepOne)
                    .build(),
            )?;
            log::info!(
                "cohort app started; logging to {} (cohort crates at {cohort_level})",
                log_dir.join("app.log").display()
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            suggest_artifacts,
            env_fingerprint,
            snapshot_artifacts,
            capture_window,
            ssh_public_key,
            ssh_target_suggestion,
            install_ssh_key,
            open_ssh,
            assistant_presets,
            assistant_config_get,
            assistant_config_set,
            assistant_config_test,
            draft_insights
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
