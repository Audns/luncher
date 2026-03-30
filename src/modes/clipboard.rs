use crate::app;
use crate::clipboard::client;
use crate::clipboard::models::{EntryKind, EntryMeta};
use crate::config::Entry;
use crate::search::LauncherItem;
use chrono::{Local, TimeZone};

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
    client::get_clipboard_history(HISTORY_LIMIT).await
}

fn entries_to_items(entries: Vec<EntryMeta>) -> Vec<LauncherItem> {
    entries
        .into_iter()
        .map(|entry| {
            let kind = display_kind(&entry);
            let kind_tag = match kind {
                EntryKind::Text => "text",
                EntryKind::Image => "image",
                EntryKind::Sensitive => "sensitive",
                EntryKind::Binary => "binary",
            };
            let preview = match kind {
                EntryKind::Sensitive => "••••••••".to_string(),
                _ => entry.preview.clone(),
            };
            let ts = format_timestamp(entry.timestamp);

            LauncherItem {
                name: preview,
                entry: Entry {
                    name: String::new(),
                    command: entry.id.to_string(),
                    tag: vec![kind_tag.into(), ts],
                    inline_meta: Some(String::new()),
                },
            }
        })
        .collect()
}

fn display_kind(entry: &EntryMeta) -> EntryKind {
    if entry.mime_type == "text/uri-list" {
        if let Some(name) = entry.filename.as_deref() {
            let ext = name
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg"
            ) {
                return EntryKind::Image;
            }
            if matches!(
                ext.as_str(),
                "txt"
                    | "md"
                    | "json"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "rs"
                    | "c"
                    | "h"
                    | "cpp"
                    | "py"
                    | "js"
                    | "ts"
            ) {
                return EntryKind::Text;
            }
        }

        return EntryKind::Binary;
    }

    entry.kind
}

fn format_timestamp(timestamp_micros: u64) -> String {
    let secs = (timestamp_micros / 1_000_000) as i64;
    let nanos = ((timestamp_micros % 1_000_000) * 1_000) as u32;

    Local
        .timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.format("%m-%d-%H:%M").to_string())
        .unwrap_or_else(|| "unknown date".to_string())
}
