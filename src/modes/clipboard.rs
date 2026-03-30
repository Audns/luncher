use crate::app;
use crate::clipboard::client;
use crate::clipboard::models::{EntryKind, EntryMeta};
use crate::config::Entry;
use crate::search::LauncherItem;

const HISTORY_LIMIT: usize = 50;

pub fn run(rt: tokio::runtime::Runtime) {
    rt.handle().spawn(async {
        let _ = client::ensure_daemon().await;
    });
    app::run(
        Vec::new(),
        false,
        true,
        Some(app::RemoteSource::Clipboard),
        Some(rt.handle().clone()),
        Some(rt),
    );
}

pub async fn load_items() -> Result<Vec<LauncherItem>, String> {
    let entries = load_history().await?;
    Ok(entries_to_items(entries))
}

async fn load_history() -> Result<Vec<EntryMeta>, String> {
    client::get_clipboard_history(HISTORY_LIMIT)
        .await
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
