use crate::app;
use crate::config::Entry;
use crate::search::LauncherItem;
use clipbowl_lib::{EntryKind, EntryMeta};
use std::path::PathBuf;

const HISTORY_LIMIT: usize = 50;
const DAEMON_WAIT_ATTEMPTS: usize = 20;
const DAEMON_WAIT_STEP_MS: u64 = 100;

pub fn run(rt: tokio::runtime::Runtime) {
    app::run(Vec::new(), false, true, Some(rt.handle().clone()), Some(rt));
}

async fn ensure_daemon() {
    if clipbowl_lib::Client::is_daemon_running().await {
        return;
    }

    // Try systemd first
    let ok = std::process::Command::new("systemctl")
        .args(["--user", "start", "clipbowl-daemon"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok {
        let _ = spawn_daemon_process();
    }

    for _ in 0..DAEMON_WAIT_ATTEMPTS {
        if clipbowl_lib::Client::is_daemon_running().await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(DAEMON_WAIT_STEP_MS)).await;
    }
}

fn spawn_daemon_process() -> std::io::Result<std::process::Child> {
    let mut last_err = None;

    for candidate in daemon_candidates() {
        let mut cmd = std::process::Command::new(&candidate);
        match cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(err) => last_err = Some(err),
        }
    }

    Err(last_err.unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "clipbowl-daemon not found")))
}

fn daemon_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("clipbowl-daemon")];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../../../clipbowl/target/debug/clipbowl-daemon"));
            candidates.push(dir.join("../../../clipbowl/target/release/clipbowl-daemon"));
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("../clipbowl/target/debug/clipbowl-daemon"));
    candidates.push(manifest_dir.join("../clipbowl/target/release/clipbowl-daemon"));

    candidates
}

pub async fn load_items() -> Result<Vec<LauncherItem>, String> {
    ensure_daemon().await;
    let entries = load_history().await?;
    Ok(entries_to_items(entries))
}

async fn load_history() -> Result<Vec<EntryMeta>, String> {
    clipbowl_lib::get_history(HISTORY_LIMIT)
        .await
        .map_err(|err| err.to_string())
}

fn entries_to_items(entries: Vec<EntryMeta>) -> Vec<LauncherItem> {
    entries
        .into_iter()
        .map(|entry| {
            let (preview, tag) = match display_kind(&entry) {
                EntryKind::Text => (entry.preview.clone(), vec!["text".into()]),
                EntryKind::Image => (entry.preview.clone(), vec!["image".into()]),
                EntryKind::Sensitive => ("••••••••".to_string(), vec!["sensitive".into()]),
                EntryKind::Binary => (entry.preview.clone(), vec!["binary".into()]),
            };

            // let age = format_age(entry.timestamp);

            LauncherItem {
                name: preview.clone(),
                entry: Entry {
                    // Store the entry id in command — used on selection
                    name: preview,
                    command: entry.id.to_string(),
                    tag,
                },
            }
        })
        .collect()
}

fn display_kind(entry: &EntryMeta) -> EntryKind {
    if entry.mime_type == "text/uri-list" {
        if let Some(name) = entry.filename.as_deref() {
            let ext = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg"
            ) {
                return EntryKind::Image;
            }
            if matches!(
                ext.as_str(),
                "txt" | "md" | "json" | "toml" | "yaml" | "yml" | "rs" | "c" | "h"
                    | "cpp" | "py" | "js" | "ts"
            ) {
                return EntryKind::Text;
            }
        }

        return EntryKind::Binary;
    }

    entry.kind
}
